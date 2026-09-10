use parking_lot::Mutex;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CAP: usize = 500;

thread_local! {
    static FROM_BUS: Cell<bool> = const { Cell::new(false) };
}

pub fn tracing_from_bus() -> bool {
    FROM_BUS.with(Cell::get)
}

fn emit_tracing(level: u32, message: &str) {
    FROM_BUS.with(|flag| {
        let prev = flag.replace(true);
        match level {
            0 => tracing::error!("{message}"),
            1 => tracing::warn!("{message}"),
            2 => tracing::info!("{message}"),
            _ => tracing::debug!("{message}"),
        }
        flag.set(prev);
    });
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub ts_us: u64,
    pub level: u32,
    pub node: Option<u64>,
    pub plugin: Option<String>,
    pub message: String,
}

impl LogLine {
    pub fn format_text(&self) -> String {
        let mut out = format!("{}  {:<5}", format_ts(self.ts_us), level_name(self.level));
        if let Some(n) = self.node {
            out.push_str(&format!("  node:{n}"));
        }
        if let Some(p) = self.plugin.as_deref().filter(|s| !s.is_empty()) {
            out.push_str("  ");
            out.push_str(p);
        }
        out.push_str("  ");
        out.push_str(&self.message);
        out
    }
}

/// A consistent slice of the log, including the retention boundary for consumers.
pub struct LogUpdate {
    pub seq: u64,
    pub retained_from: u64,
    pub lines: Vec<(u64, LogLine)>,
}

#[derive(Clone, Default)]
pub struct LogBus {
    inner: Arc<Mutex<VecDeque<LogLine>>>,
    seq: Arc<AtomicU64>,
}

impl LogBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    pub fn push(&self, line: LogLine) {
        let mut g = self.inner.lock();
        if g.len() >= CAP {
            g.pop_front();
        }
        g.push_back(line);
        // Publish the cursor while holding the same lock as the buffer so an
        // incremental reader cannot associate lines with the wrong sequence.
        self.seq.fetch_add(1, Ordering::Release);
    }

    pub fn log(&self, level: u32, node: Option<u64>, message: impl Into<String>) {
        let message = message.into();
        self.push(LogLine {
            ts_us: now_us(),
            level,
            node,
            plugin: None,
            message: message.clone(),
        });
        emit_tracing(level, &message);
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.inner.lock().iter().cloned().collect()
    }

    pub fn snapshot_since(&self, after: u64) -> LogUpdate {
        let lines = self.inner.lock();
        let seq = self.seq.load(Ordering::Relaxed);
        let retained_from = seq - lines.len() as u64 + 1;
        let skip = after
            .saturating_sub(retained_from - 1)
            .min(lines.len() as u64) as usize;
        LogUpdate {
            seq,
            retained_from,
            lines: lines
                .iter()
                .enumerate()
                .skip(skip)
                .map(|(i, line)| (retained_from + i as u64, line.clone()))
                .collect(),
        }
    }
}

pub fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

pub fn format_ts(ts_us: u64) -> String {
    let secs = (ts_us / 1_000_000) as i64;
    let micros = ts_us % 1_000_000;
    let (year, month, day, hour, min, sec) = civil_utc(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{micros:06}Z")
}

/// Days since 1970-01-01 → UTC civil date. Howard Hinnant `civil_from_days`.
fn civil_utc(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

pub fn level_name(level: u32) -> &'static str {
    match level {
        0 => "ERROR",
        1 => "WARN",
        2 => "INFO",
        _ => "DEBUG",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_reads_handle_retention_and_identical_timestamps() {
        let bus = LogBus::new();
        let empty = bus.snapshot_since(0);
        assert_eq!(empty.seq, 0);
        assert!(empty.lines.is_empty());
        for i in 0..CAP + 3 {
            bus.push(LogLine {
                ts_us: 1,
                level: 2,
                node: None,
                plugin: None,
                message: i.to_string(),
            });
        }
        let update = bus.snapshot_since(0);
        assert_eq!(update.retained_from, 4);
        assert_eq!(update.lines.len(), CAP);
        assert_eq!(update.lines[0].0, 4);
        assert_eq!(update.lines[0].1.message, "3");
        let tail = bus.snapshot_since(update.seq - 1);
        assert_eq!(tail.lines.len(), 1);
        assert_eq!(tail.lines[0].0, update.seq);
        assert!(bus.snapshot_since(update.seq).lines.is_empty());
    }

    #[test]
    fn concurrent_reads_observe_consistent_buffer_cursors() {
        let bus = LogBus::new();
        let other = bus.clone();
        let writer = std::thread::spawn(move || {
            for i in 1..2000 {
                other.push(LogLine {
                    ts_us: 1,
                    level: 2,
                    node: None,
                    plugin: None,
                    message: i.to_string(),
                });
            }
        });
        let mut cursor = 0;
        while cursor < 1999 {
            let update = bus.snapshot_since(cursor);
            for (seq, line) in &update.lines {
                assert_eq!(*seq, line.message.parse::<u64>().unwrap());
            }
            if let Some((last, _)) = update.lines.last() {
                assert_eq!(*last, update.seq);
            }
            cursor = update.seq;
            std::thread::yield_now();
        }
        writer.join().unwrap();
    }

    #[test]
    fn format_unix_epoch() {
        assert_eq!(format_ts(0), "1970-01-01T00:00:00.000000Z");
    }

    #[test]
    fn format_known_instant() {
        assert_eq!(
            format_ts(1_789_023_372_123_456),
            "2026-09-10T06:56:12.123456Z"
        );
    }

    #[test]
    fn seq_advances_on_push() {
        let bus = LogBus::new();
        assert_eq!(bus.seq(), 0);
        bus.log(2, None, "a");
        bus.log(2, None, "b");
        assert_eq!(bus.seq(), 2);
    }

    #[test]
    fn format_line_keeps_timestamp() {
        let line = LogLine {
            ts_us: 0,
            level: 2,
            node: None,
            plugin: None,
            message: "session ready".into(),
        };
        assert_eq!(
            line.format_text(),
            "1970-01-01T00:00:00.000000Z  INFO   session ready"
        );
    }
}
