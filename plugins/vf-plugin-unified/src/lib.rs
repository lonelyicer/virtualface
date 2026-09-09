mod arkit;
mod calib;
mod correctors;
mod filter;
mod math;
mod merge;

pub use arkit::{ArkitMapper, ArkitToUnified};
pub use calib::Calibration;
pub use correctors::Correctors;
pub use filter::OneEuroFilter;
pub use math::ShapeMath;
pub use merge::MergeUnified;

vf_sdk::export_plugin! {
    id: "vf.unified",
    name: "Unified Processing",
    version: "0.1.0",
    nodes: [ArkitToUnified, Calibration, OneEuroFilter, MergeUnified, ShapeMath, Correctors]
}
