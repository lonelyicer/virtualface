//! Coalesced, ordered autosaves. Only cold archive operations and shutdown wait
//! for the writer; typing never serializes JSON or touches the filesystem.

use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use vf_core::{Graph, LogBus, Snapshot, graph_to_json, write_archive_json};

struct Save {
    graph: Graph,
    snapshot: Arc<Snapshot>,
}

enum Command {
    Save(String, Save),
    Flush(mpsc::Sender<()>),
    Shutdown,
}

pub(crate) struct Autosave {
    tx: mpsc::Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Autosave {
    pub fn new(log: LogBus) -> Self {
        Self::with_writer(log, |id, graph| {
            graph_to_json(graph)
                .and_then(|json| write_archive_json(id, &json))
                .map_err(|err| err.to_string())
        })
    }

    fn with_writer(
        log: LogBus,
        mut write: impl FnMut(&str, &Graph) -> Result<(), String> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("vf-autosave".into())
            .spawn(move || {
                let mut pending: HashMap<String, Save> = HashMap::new();
                let mut flush = |pending: &mut HashMap<String, Save>| {
                    for (id, mut save) in pending.drain() {
                        for node in &mut save.graph.nodes {
                            if let Some(snap) = save.snapshot.nodes.get(&node.id.0) {
                                node.state = snap.state.clone();
                            }
                        }
                        if let Err(err) = write(&id, &save.graph) {
                            log.log(0, None, format!("autosave {id} failed: {err}"));
                        }
                    }
                };
                loop {
                    let command = if pending.is_empty() {
                        rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    } else {
                        rx.recv_timeout(Duration::from_millis(250))
                    };
                    match command {
                        Ok(Command::Save(id, save)) => {
                            pending.insert(id, save);
                        }
                        Ok(Command::Flush(done)) => {
                            flush(&mut pending);
                            let _ = done.send(());
                        }
                        Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            flush(&mut pending);
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => flush(&mut pending),
                    }
                }
            })
            .expect("spawn autosave thread");
        Self {
            tx,
            thread: Some(thread),
        }
    }

    pub fn save(&self, id: String, graph: Graph, snapshot: Arc<Snapshot>) {
        let _ = self.tx.send(Command::Save(id, Save { graph, snapshot }));
    }

    /// Barrier before synchronous save/load/delete, so an older queued write
    /// cannot overwrite newer data or recreate a deleted archive.
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Command::Flush(tx)).is_ok() {
            let _ = rx.recv();
        }
    }
}

impl Drop for Autosave {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn flush_orders_latest_versions_before_archive_operations() {
        let saved = Arc::new(Mutex::new(HashMap::new()));
        let sink = saved.clone();
        let writer = Autosave::with_writer(LogBus::new(), move |id, graph| {
            sink.lock().unwrap().insert(id.to_string(), graph.clone());
            Ok(())
        });
        let snap = Arc::new(Snapshot::default());
        writer.save("a".into(), Graph::default(), snap.clone());
        let latest = Graph {
            next_id: 42,
            ..Graph::default()
        };
        writer.save("a".into(), latest.clone(), snap.clone());
        writer.save("b".into(), Graph::default(), snap);
        writer.flush();
        assert_eq!(saved.lock().unwrap()["a"], latest);
        assert!(saved.lock().unwrap().contains_key("b"));
        // Simulate deleting an archive after the barrier. Shutdown must not recreate it.
        saved.lock().unwrap().remove("a");
        drop(writer);
        assert!(!saved.lock().unwrap().contains_key("a"));
    }

    #[test]
    fn shutdown_drains_pending_save_and_reports_write_errors() {
        let log = LogBus::new();
        let writer = Autosave::with_writer(log.clone(), |_, _| Err("disk full".into()));
        writer.save("a".into(), Graph::default(), Arc::new(Snapshot::default()));
        drop(writer);
        assert!(
            log.snapshot()
                .iter()
                .any(|line| line.message.contains("disk full"))
        );
    }
}
