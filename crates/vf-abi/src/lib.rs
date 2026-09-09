//! Stable C ABI for VirtualFace plugins.
//!
//! This crate has no dependencies. Layout is `#[repr(C)]` and versioned by
//! [`VF_ABI_VERSION`]. Hosts load `vf_plugin_entry` from a cdylib.

#![allow(clippy::missing_safety_doc)]

pub mod plugin;
pub mod unified;
pub mod value;

pub use plugin::*;
pub use unified::*;
pub use value::*;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn unified_expression_order_matches_vrcft() {
        assert_eq!(UnifiedExpression::EyeSquintRight as u32, 0);
        assert_eq!(UnifiedExpression::JawOpen as u32, 22);
        assert_eq!(UnifiedExpression::TongueOut as u32, 72);
        assert_eq!(UnifiedExpression::NeckFlexLeft as u32, 87);
        assert_eq!(UnifiedExpression::Max as u32, 88);
        assert_eq!(VF_UNIFIED_SHAPE_COUNT, 89);
        assert_eq!(UnifiedExpression::ALL.len(), 88);
        assert_eq!(
            UnifiedExpression::from_name("JawOpen"),
            Some(UnifiedExpression::JawOpen)
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn abi_sizes_align_64() {
        assert_eq!(align_of::<VfPluginDescriptor>(), 8);
        assert_eq!(size_of::<VfPluginDescriptor>(), 48);
        assert_eq!(size_of::<VfPortDesc>(), 32);
        assert_eq!(size_of::<VfNodeDescriptor>(), 80);
        assert_eq!(size_of::<VfProcessCtx>(), 24);
        assert_eq!(size_of::<VfHostApi>(), 32);
        assert_eq!(size_of::<VfValueTag>(), 4);
        assert_eq!(size_of::<VfValue>(), 32);
        assert_eq!(size_of::<VfFloatBuf>(), 24);
        assert_eq!(size_of::<VfEyeSample>(), 16);
        assert_eq!(size_of::<VfEyeData>(), 32);
        assert_eq!(size_of::<VfHeadData>(), 24);
        assert_eq!(offset_of!(VfUnifiedFrame, shapes), 8 + 4 + 4 + 32 + 24);
        assert_eq!(
            size_of::<VfUnifiedFrame>(),
            8 + 4 + 4 + 32 + 24 + 4 * VF_UNIFIED_SHAPE_COUNT + 4 // tail pad to align 8
        );
        assert_eq!(align_of::<VfUnifiedFrame>(), 8);
        assert_eq!(size_of::<VfNodeVTable>(), 64);
        assert_eq!(size_of::<VfNodeStatus>(), 260);
    }

    #[test]
    fn ports_compatible_schema() {
        assert!(ports_compatible(
            VfValueTag::Float,
            None,
            VfValueTag::Float,
            None
        ));
        assert!(!ports_compatible(
            VfValueTag::Float,
            None,
            VfValueTag::Int,
            None
        ));
        assert!(ports_compatible(
            VfValueTag::Blendshapes,
            Some("arkit52"),
            VfValueTag::Blendshapes,
            Some("arkit52")
        ));
        assert!(!ports_compatible(
            VfValueTag::Blendshapes,
            Some("arkit52"),
            VfValueTag::Blendshapes,
            Some("pico72")
        ));
        assert!(ports_compatible(
            VfValueTag::Blendshapes,
            Some("arkit52"),
            VfValueTag::Blendshapes,
            Some("*")
        ));
    }
}
