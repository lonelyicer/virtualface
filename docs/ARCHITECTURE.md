# VirtualFace architecture

The host is a graph runner. Face-tracking semantics live in plugins.

## Crates

| Crate | Role |
| --- | --- |
| `vf-abi` | Versioned C ABI (`VF_ABI_VERSION = 1`). Zero dependencies. |
| `vf-sdk` | Safe `Node` trait + `export_plugin!` for plugin authors. |
| `vf-core` | `libloading` host, graph compile (`petgraph` toposort), engine thread. |
| `vf-ui` | gpui-kit workspace: palette, node canvas, inspector, log. |
| `vf-app` | `virtualface` binary (`--headless`, `--graph`, `--plugins-dir`). |

Example plugins (one crate may export any mix of node categories):

- `vf-plugin-pico` — `pico.udp_source`, `pico.synthetic`
- `vf-plugin-unified` — calibration, 1€ filter, merge, shape math, correctors
- `vf-plugin-vrc-osc` — VRChat `v2/*` OSC, raw float OSC

## Plugin ABI

Each cdylib exports:

```c
const VfPluginDescriptor* vf_plugin_entry(uint32_t host_abi);
```

If `host_abi != VF_ABI_VERSION` the function returns null and the host skips the library.

Hot path: host-owned `VfValue` buffers, `process(ctx, inputs, outputs)`. Cold path: JSON for params/state.

`VfHostApi` provides `log`, `wake` (input threads), and `now_us`. `user_data` is required so callbacks can reach the engine.

## Intermediate protocol

- `UnifiedFrame` — VRCFT `UnifiedTrackingData` layout (`VfEyeData` + `shapes[89]` + `VfHeadData`). Shape indices match VRCFT `UnifiedExpressions`.
- Scalars `float` / `int` / `bool` / `vec2` / `vec3`.

Wires require matching `VfValueTag`. One edge per input port. Cycles are rejected.

## Engine

A dedicated thread ticks at 100 Hz. Input nodes call `host.wake()` to cut wait. Node instances are keyed by `NodeId` and reused across recompiles so calibration state survives reconnects.

`Session` shuts the engine down (join + `destroy`) **before** `PluginHost` drops `libloading::Library` handles. Unmapping a cdylib while vtable pointers are still in use is undefined (typically SIGSEGV). Plugins are not unloaded at runtime.

Snapshots are published through `ArcSwap` for a 30 Hz UI poll.

## UI

`vf-ui` is a gpui-kit `Root` window with a fixed split (Phase 2 can persist this as `DockArea`):

- Toolbar: Start / Stop / Open / Save / Reload / Delete, plus tick and drop stats
- Left palette: node types grouped by plugin, plus loaded plugin list
- Center canvas: grid, cubic-bezier wires, pan/zoom, drag nodes, typed connections
- Right inspector: schema-driven params, live port previews, status
- Bottom log: plugin `host.log` ring buffer

## Plugin search paths

The host scans `plugins/`, `target/debug`, `target/release`, the executable directory, and extra `--plugins-dir` folders. A file is loaded if it is a `*.so` / `*.dll` / `*.dylib` whose name contains `vf_plugin` / `vf-plugin`, or that sits directly in a `plugins/` directory.

## Writing a plugin

```rust
use vf_sdk::*;

pub struct Gain { k: f32 }

impl Node for Gain {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("demo.gain", "Gain", Category::Process)
            .input(PortDesc::unified("in"))
            .output(PortDesc::unified("out"))
            .param(ParamDef::float("k", "Gain", 1.0, 0.0, 4.0, 0.01))
    }
    fn create(_host: Host, cfg: &serde_json::Value) -> Result<Self> {
        Ok(Self { k: param_f32(cfg, "k", 1.0) })
    }
    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let mut f = *io.input_unified(0)?;
        for s in &mut f.shapes { *s *= self.k; }
        *io.output_unified_mut(0)? = f;
        Ok(())
    }
    fn set_param(&mut self, key: &str, v: &serde_json::Value) -> Result<()> {
        if key == "k" { self.k = json_f64(v, 1.0) as f32; }
        Ok(())
    }
}

export_plugin! {
    id: "demo",
    name: "Demo",
    version: "0.1.0",
    nodes: [Gain]
}
```

Set `crate-type = ["cdylib", "rlib"]`. Drop the resulting `lib*.so` / `*.dll` / `lib*.dylib` in a scanned plugin directory.

ABI changes: only append fields and bump `VF_ABI_VERSION`. The host refuses a mismatched major.
