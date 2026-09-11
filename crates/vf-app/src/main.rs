mod log_layer;

use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use vf_core::{
    Graph, LogBus, Session, absolute_path, default_plugin_dirs, graph_from_json, last_graph_path,
    load_archive_index, read_archive_json,
};

use crate::log_layer::LogBusLayer;

#[derive(Parser, Debug)]
#[command(name = "virtualface", about = "Plugin-based face tracking host")]
struct Cli {
    /// Extra directories to scan for plugins (cdylib).
    #[arg(long = "plugins-dir")]
    plugins_dir: Vec<PathBuf>,
    /// Graph file to load (.vfgraph.json).
    #[arg(long)]
    graph: Option<PathBuf>,
    /// Run the engine without opening a window.
    #[arg(long)]
    headless: bool,
}

fn main() {
    let log = LogBus::new();
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with(tracing_subscriber::fmt::layer())
        .with(
            LogBusLayer::new(log.clone()).with_filter(
                EnvFilter::new("info")
                    .add_directive("virtualface=info".parse().unwrap())
                    .add_directive("vf_core=info".parse().unwrap())
                    .add_directive("vf_ui=info".parse().unwrap())
                    .add_directive("gpui=warn".parse().unwrap())
                    .add_directive("gpui_kit=warn".parse().unwrap()),
            ),
        )
        .init();

    let cli = Cli::parse();
    let mut dirs = cli.plugins_dir.clone();
    dirs.extend(default_plugin_dirs());

    let mut loaded = Graph::default();
    let mut file_path = None;
    let mut archive_id = None;
    let mut graph_label = String::new();
    if let Some(path) = cli.graph.clone() {
        match std::fs::read_to_string(&path) {
            Ok(s) => match graph_from_json(&s) {
                Ok(g) => {
                    loaded = g;
                    graph_label = path.display().to_string();
                    file_path = Some(path);
                }
                Err(e) => {
                    log.log(
                        0,
                        None,
                        format!("failed to parse graph {}: {e}", path.display()),
                    );
                }
            },
            Err(e) => {
                log.log(
                    0,
                    None,
                    format!("failed to read graph {}: {e}", path.display()),
                );
            }
        }
    } else {
        let index = load_archive_index();
        if let Some(meta) = index.current_meta() {
            match read_archive_json(&meta.id) {
                Ok(s) => match graph_from_json(&s) {
                    Ok(g) => {
                        loaded = g;
                        graph_label = meta.name.clone();
                        archive_id = Some(meta.id.clone());
                    }
                    Err(e) => {
                        log.log(
                            0,
                            None,
                            format!("failed to parse archive {}: {e}", meta.name),
                        );
                    }
                },
                Err(e) => {
                    log.log(
                        0,
                        None,
                        format!("failed to read archive {}: {e}", meta.name),
                    );
                }
            }
        } else if let Some(path) = last_graph_path() {
            match std::fs::read_to_string(&path) {
                Ok(s) => match graph_from_json(&s) {
                    Ok(g) => {
                        loaded = g;
                        graph_label = path.display().to_string();
                        file_path = Some(path);
                    }
                    Err(e) => {
                        log.log(
                            0,
                            None,
                            format!("failed to parse graph {}: {e}", path.display()),
                        );
                    }
                },
                Err(e) => {
                    log.log(
                        0,
                        None,
                        format!("failed to read graph {}: {e}", path.display()),
                    );
                }
            }
        }
    }

    let session = Session::boot_with_log(&dirs, Some(loaded), log);
    if let Some(path) = file_path {
        *session.graph_path.lock() = absolute_path(&path).display().to_string();
    } else if let Some(id) = archive_id {
        *session.graph_path.lock() = id;
    }
    session.host.log.log(
        2,
        None,
        if graph_label.is_empty() {
            "graph (untitled)".into()
        } else {
            format!("graph {graph_label}")
        },
    );

    if cli.headless {
        run_headless(session);
    } else {
        run_gui(session);
    }
}

fn run_headless(session: Session) {
    session.engine.start();
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = stop.clone();
    let _ = ctrlc::set_handler(move || s2.store(true, Ordering::SeqCst));
    session
        .host
        .log
        .log(2, None, "headless engine running — Ctrl+C to stop");
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(200));
        let snap = session.engine.snapshot();
        if snap.tick.is_multiple_of(100) && snap.tick > 0 {
            session.host.log.log(
                2,
                None,
                format!("engine tick={} drops={}", snap.tick, snap.drops),
            );
        }
    }
    session.engine.stop();
}

fn run_gui(session: Session) {
    let session = Arc::new(session);
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        vf_ui::init(cx);
        vf_ui::open_workspace(session, cx);
    });
}
