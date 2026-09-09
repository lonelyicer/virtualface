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

CLI flags: `--plugins-dir`, `--graph`, `--headless`, `--rate` (Hz, default 100).

On Fedora/RHEL, GUI linking needs `libxkbcommon-x11`. The runtime `.so.0` is enough; `libxkbcommon-x11-devel` is the usual package if you prefer not to rely on the app `build.rs` workaround.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the C ABI, SDK, and plugin authoring guide.
