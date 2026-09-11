mod packet;
mod remap;
mod source;
mod ue;

pub use source::PicoUdpSource;

vf_sdk::export_plugin! {
    id: "vf.pico",
    name: "PICO Input",
    version: "0.1.0",
    description: "PICO Connect UDP face-tracking source",
    author: "VirtualFace contributors",
    license: "PolyForm-Noncommercial-1.0.0",
    homepage: "https://github.com/lonelyicer/virtualface",
    repository: "https://github.com/lonelyicer/virtualface",
    issues: "https://github.com/lonelyicer/virtualface/issues",
    keywords: "pico, input, udp, face-tracking",
    nodes: [PicoUdpSource]
}
