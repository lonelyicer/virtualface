use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use vf_core::{LogBus, LogLine, now_us, tracing_from_bus};

pub struct LogBusLayer {
    bus: LogBus,
}

impl LogBusLayer {
    pub fn new(bus: LogBus) -> Self {
        Self { bus }
    }
}

impl<S> Layer<S> for LogBusLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        if tracing_from_bus() {
            return;
        }
        let mut visitor = Fields::default();
        event.record(&mut visitor);
        let mut message = visitor.message;
        if !visitor.rest.is_empty() {
            if !message.is_empty() {
                message.push(' ');
            }
            message.push_str(&visitor.rest);
        }
        if message.is_empty() {
            message = event.metadata().target().to_string();
        }
        self.bus.push(LogLine {
            ts_us: now_us(),
            level: tracing_level(event.metadata().level()),
            node: visitor.node,
            plugin: None,
            message,
        });
    }
}

fn tracing_level(level: &Level) -> u32 {
    match *level {
        Level::ERROR => 0,
        Level::WARN => 1,
        Level::INFO => 2,
        _ => 3,
    }
}

#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
    node: Option<u64>,
}

impl Fields {
    fn add(&mut self, name: &str, value: String) {
        if name == "message" {
            self.message = unquote(value);
            return;
        }
        if name == "node" {
            if let Ok(n) = value.parse::<u64>() {
                self.node = Some(n);
                return;
            }
        }
        if !self.rest.is_empty() {
            self.rest.push(' ');
        }
        self.rest.push_str(name);
        self.rest.push('=');
        self.rest.push_str(&unquote(value));
    }
}

fn unquote(s: String) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s
    }
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.add(field.name(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.add(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "node" {
            self.node = Some(value);
        } else {
            self.add(field.name(), value.to_string());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.add(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.add(field.name(), value.to_string());
    }
}
