# VirtualFace

A native (gpui) face-tracking host. The app is a graph runner: it loads nodes from plugins and lets you wire **input → process → output**. Face-tracking semantics live in those plugins, not in the executable.

A typical path is **Device → Unified Expressions → VRChat OSC**.

## Bundled plugins

| Plugin | Nodes |
| --- | --- |
| `vf-plugin-pico` | PICO Connect UDP source |
| `vf-plugin-unified` | Calibration, One Euro Filter, merge, shape math, correctors |
| `vf-plugin-vrc-osc` | VRChat `v2/*` OSC output, raw float OSC |

Third-party plugins are cdylibs that export `vf_plugin_entry`. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the ABI, SDK, and a minimal authoring example. A standalone starter crate lives at [vf-plugin-template](https://github.com/lonelyicer/vf-plugin-template).

## Build and run

```bash
cargo build --workspace

# GUI (scans plugins/, the executable directory, and target/<profile>/)
cargo run -p vf-app -- --graph examples/graphs/pico_to_vrchat.vfgraph.json

# Headless engine
cargo run -p vf-app -- --headless --graph examples/graphs/pico_to_vrchat.vfgraph.json

# Extra plugin directories
cargo run -p vf-app -- --plugins-dir ./my-plugins
```

Use `--release` when you want a performance baseline.

### CLI

| Flag | Meaning |
| --- | --- |
| `--graph <path>` | Load a `.vfgraph.json` |
| `--headless` | Run the engine without a window |
| `--plugins-dir <path>` | Extra directory to scan for plugins (repeatable) |

### Tests

Build plugin libraries first so workspace integration tests can load them:

```bash
cargo build --workspace
cargo test --workspace
```

## License

VirtualFace is licensed under [PolyForm Noncommercial 1.0.0](LICENSE.md).

## Credits

- [VRCFaceTracking](https://github.com/benaclejames/VRCFaceTracking) — Unified Expressions layout and names, rolling-window calibration, One Euro-style filtering, `v2` OSC parameter derivation, OSCQuery / avatar JSON parameter selection

Those projects are not affiliated with VirtualFace. Their names, trademarks, and licenses remain with their authors.

The One Euro Filter algorithm is from Casiez, Roussel, and Vogel, *1 € Filter: A Simple Speed-based Low-pass Filter for Noisy Input in Interactive Systems* (CHI 2012).
