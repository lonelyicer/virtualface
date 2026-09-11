mod calib;
mod correctors;
mod filter;
mod math;
mod merge;

pub use calib::Calibration;
pub use correctors::Correctors;
pub use filter::OneEuroFilter;
pub use math::ShapeMath;
pub use merge::MergeUnified;

vf_sdk::export_plugin! {
    id: "vf.unified",
    name: "Unified Processing",
    version: "0.1.0",
    description: "Calibration, One Euro filter, merge, shape math, and correctors",
    author: "VirtualFace contributors",
    license: "PolyForm-Noncommercial-1.0.0",
    homepage: "https://github.com/lonelyicer/virtualface",
    repository: "https://github.com/lonelyicer/virtualface",
    issues: "https://github.com/lonelyicer/virtualface/issues",
    keywords: "unified, process, filter, merge",
    nodes: [Calibration, OneEuroFilter, MergeUnified, ShapeMath, Correctors]
}
