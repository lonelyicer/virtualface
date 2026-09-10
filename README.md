# VirtualFace

A plugin-hosted face-tracking graph: the executable loads nodes from dynamic libraries and lets you wire **input → process → output** (PICO → Unified Expressions → VRChat OSC).

```bash
# GUI (loads plugins from plugins/ and target/<profile>/)
cargo run -p vf-app -- --graph examples/graphs/pico_to_vrchat.vfgraph.json

# Headless engine
cargo run -p vf-app -- --headless --graph examples/graphs/pico_to_vrchat.vfgraph.json

# Extra plugin directories
cargo run -p vf-app -- --plugins-dir ./my-plugins --headless
```

Development builds optimize the GPUI/Taffy dependencies while keeping application code debuggable. For a release performance baseline, run `cargo run --release -p vf-app -- --graph examples/graphs/pico_to_vrchat.vfgraph.json`. Set `ZED_MEASUREMENTS=1` to print GPUI frame timings; measure while interacting with a focused window, since idle windows render on demand.

Build the plugin libraries before running the integration tests:

```bash
cargo build --workspace
cargo test --workspace
```

CLI flags: `--plugins-dir`, `--graph`, `--headless`.

On Fedora/RHEL, GUI linking needs `libxkbcommon-x11`. The app `build.rs` can use the runtime `.so.0`; linking the `vf-ui` test binary also requires `libxkbcommon-x11-devel` or a `LIBRARY_PATH` directory containing a `libxkbcommon-x11.so` symlink to that runtime library.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the C ABI, SDK, and plugin authoring guide.
