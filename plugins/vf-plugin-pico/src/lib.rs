mod packet;
mod remap;
mod source;
mod ue;

pub use source::PicoUdpSource;

vf_sdk::export_plugin! {
    id: "vf.pico",
    name: "PICO Input",
    version: "0.1.0",
    nodes: [PicoUdpSource]
}
