use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use vf_core::{Graph, PluginHost, Session, default_plugin_dirs, graph_from_json};

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
    /// Engine tick rate (Hz).
    #[arg(long, default_value_t = 100.0)]
    rate: f32,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let cli = Cli::parse();
    let mut dirs = cli.plugins_dir.clone();
    dirs.extend(default_plugin_dirs());

    let mut loaded = match &cli.graph {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(s) => match graph_from_json(&s) {
                Ok(g) => g,
                Err(e) => {
                    tracing::error!(file = %path.display(), error = %e, "failed to parse graph");
                    Graph::default()
                }
            },
            Err(e) => {
                tracing::error!(file = %path.display(), error = %e, "failed to read graph");
                Graph::default()
            }
        },
        None => Graph::default(),
    };
    if cli.rate > 0.0 {
        loaded.rate_hz = cli.rate;
    }

    let session = Session::boot(&dirs, Some(loaded));
    if let Some(path) = &cli.graph {
        *session.graph_path.lock() = path.display().to_string();
    }
    tracing::info!(
        plugins = session.host.plugins().len(),
        nodes = session.registry.all().len(),
        "session ready"
    );
    session.host.log.log(
        2,
        None,
        format!("graph {}", session.graph_path.lock().clone()),
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
    tracing::info!("headless engine running — Ctrl+C to stop");
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(200));
        let snap = session.engine.snapshot();
        if snap.tick % 100 == 0 && snap.tick > 0 {
            tracing::info!(tick = snap.tick, drops = snap.drops, "engine");
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

// keep PluginHost referenced for docs
#[allow(dead_code)]
fn _host() -> PluginHost {
    PluginHost::new(vf_core::LogBus::new())
}
