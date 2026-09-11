mod avatar_cfg;
mod mdns_vrc;
mod oscquery;
mod output;
mod params;
mod raw;

pub use output::{VrcOscOutput, encode_float_message};
pub use params::derive_v2;
pub use raw::OscRawOutput;

vf_sdk::export_plugin! {
    id: "vf.vrc_osc",
    name: "VRChat OSC Output",
    version: "0.1.0",
    description: "Sends Unified Expressions to VRChat over OSC / OSCQuery",
    author: "VirtualFace contributors",
    license: "PolyForm-Noncommercial-1.0.0",
    homepage: "https://github.com/lonelyicer/virtualface",
    repository: "https://github.com/lonelyicer/virtualface",
    issues: "https://github.com/lonelyicer/virtualface/issues",
    keywords: "vrchat, osc, output",
    nodes: [VrcOscOutput, OscRawOutput]
}
