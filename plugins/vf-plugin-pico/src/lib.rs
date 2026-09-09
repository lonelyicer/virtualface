mod packet;
mod remap;
mod source;
mod synthetic;
mod ue;

pub use packet::{encode_new_packet, parse_pico_packet};
pub use remap::pico_visemes;
pub use source::PicoUdpSource;
pub use synthetic::SyntheticSource;

vf_sdk::export_plugin! {
    id: "vf.pico",
    name: "PICO Input",
    version: "0.1.0",
    nodes: [PicoUdpSource, SyntheticSource]
}
