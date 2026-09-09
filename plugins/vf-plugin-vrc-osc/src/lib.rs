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
    nodes: [VrcOscOutput, OscRawOutput]
}
