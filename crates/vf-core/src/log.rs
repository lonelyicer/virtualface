use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const CAP: usize = 500;

#[derive(Clone, Debug)]
pub struct LogLine {
    pub ts_us: u64,
    pub level: u32,
    pub node: Option<u64>,
    pub plugin: Option<String>,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct LogBus {
    inner: Arc<Mutex<VecDeque<LogLine>>>,
}

impl LogBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, line: LogLine) {
        let mut g = self.inner.lock();
        if g.len() >= CAP {
            g.pop_front();
        }
        g.push_back(line);
    }

    pub fn log(&self, level: u32, node: Option<u64>, message: impl Into<String>) {
        self.push(LogLine {
            ts_us: now_us(),
            level,
            node,
            plugin: None,
            message: message.into(),
        });
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.inner.lock().iter().cloned().collect()
    }
}

pub fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

pub fn level_name(level: u32) -> &'static str {
    match level {
        0 => "ERROR",
        1 => "WARN",
        2 => "INFO",
        _ => "DEBUG",
    }
}
