//! Tagged values passed between nodes on the hot path.
//!
//! All buffers (`UnifiedFrame`, bytes) are allocated and owned
//! by the host. `process` may read inputs and write outputs but must not retain
//! pointers across calls.

use crate::unified::VF_UNIFIED_SHAPE_COUNT;
use core::ptr;

pub const VF_VALID_EYE: u32 = 1 << 0;
pub const VF_VALID_EXPR: u32 = 1 << 1;
pub const VF_VALID_HEAD: u32 = 1 << 2;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VfValueTag {
    Empty = 0,
    Float = 1,
    Int = 2,
    Bool = 3,
    Vec2 = 4,
    Vec3 = 5,
    UnifiedFrame = 6,
    Bytes = 8,
}

impl VfValueTag {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Float => "float",
            Self::Int => "int",
            Self::Bool => "bool",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::UnifiedFrame => "unified",
            Self::Bytes => "bytes",
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfEyeSample {
    pub gaze: [f32; 2],
    pub openness: f32,
    pub pupil_mm: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfEyeData {
    pub left: VfEyeSample,
    pub right: VfEyeSample,
}

impl VfEyeData {
    pub fn combined_gaze(&self) -> [f32; 2] {
        [
            (self.left.gaze[0] + self.right.gaze[0]) * 0.5,
            (self.left.gaze[1] + self.right.gaze[1]) * 0.5,
        ]
    }

    pub fn combined_openness(&self) -> f32 {
        (self.left.openness + self.right.openness) * 0.5
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfHeadData {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub pos: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VfUnifiedFrame {
    pub timestamp_us: u64,
    pub valid: u32,
    pub _pad: u32,
    pub eye: VfEyeData,
    pub head: VfHeadData,
    pub shapes: [f32; VF_UNIFIED_SHAPE_COUNT],
}

impl Default for VfUnifiedFrame {
    fn default() -> Self {
        Self {
            timestamp_us: 0,
            valid: 0,
            _pad: 0,
            eye: VfEyeData::default(),
            head: VfHeadData::default(),
            shapes: [0.0; VF_UNIFIED_SHAPE_COUNT],
        }
    }
}

impl VfUnifiedFrame {
    pub fn shape(&self, expr: crate::unified::UnifiedExpression) -> f32 {
        self.shapes[expr.index()]
    }

    pub fn set_shape(&mut self, expr: crate::unified::UnifiedExpression, value: f32) {
        self.shapes[expr.index()] = value;
    }

    pub fn copy_from(&mut self, other: &Self) {
        *self = *other;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VfBytes {
    pub tag: *const core::ffi::c_char,
    pub ptr: *mut u8,
    pub len: u32,
    pub cap: u32,
}

impl VfBytes {
    pub fn empty() -> Self {
        Self {
            tag: ptr::null(),
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VfPayload {
    pub empty: u8,
    pub float: f32,
    pub int: i64,
    pub boolean: u8,
    pub vec2: [f32; 2],
    pub vec3: [f32; 3],
    pub unified: *mut VfUnifiedFrame,
    pub bytes: VfBytes,
}

impl Default for VfPayload {
    fn default() -> Self {
        Self { empty: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfValue {
    pub tag: VfValueTag,
    pub _pad: u32,
    pub payload: VfPayload,
}

unsafe impl Send for VfPayload {}
unsafe impl Sync for VfPayload {}
unsafe impl Send for VfValue {}
unsafe impl Sync for VfValue {}
unsafe impl Send for VfBytes {}
unsafe impl Sync for VfBytes {}

impl Default for VfValue {
    fn default() -> Self {
        Self {
            tag: VfValueTag::Empty,
            _pad: 0,
            payload: VfPayload::default(),
        }
    }
}

impl VfValue {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn float(v: f32) -> Self {
        Self {
            tag: VfValueTag::Float,
            _pad: 0,
            payload: VfPayload { float: v },
        }
    }

    pub fn int(v: i64) -> Self {
        Self {
            tag: VfValueTag::Int,
            _pad: 0,
            payload: VfPayload { int: v },
        }
    }

    pub fn boolean(v: bool) -> Self {
        Self {
            tag: VfValueTag::Bool,
            _pad: 0,
            payload: VfPayload {
                boolean: u8::from(v),
            },
        }
    }

    pub fn vec2(v: [f32; 2]) -> Self {
        Self {
            tag: VfValueTag::Vec2,
            _pad: 0,
            payload: VfPayload { vec2: v },
        }
    }

    pub fn vec3(v: [f32; 3]) -> Self {
        Self {
            tag: VfValueTag::Vec3,
            _pad: 0,
            payload: VfPayload { vec3: v },
        }
    }

    pub fn unified(ptr: *mut VfUnifiedFrame) -> Self {
        Self {
            tag: VfValueTag::UnifiedFrame,
            _pad: 0,
            payload: VfPayload { unified: ptr },
        }
    }

    pub fn as_float(self) -> Option<f32> {
        if self.tag == VfValueTag::Float {
            Some(unsafe { self.payload.float })
        } else {
            None
        }
    }

    pub fn as_int(self) -> Option<i64> {
        if self.tag == VfValueTag::Int {
            Some(unsafe { self.payload.int })
        } else {
            None
        }
    }

    pub fn as_bool(self) -> Option<bool> {
        if self.tag == VfValueTag::Bool {
            Some(unsafe { self.payload.boolean } != 0)
        } else {
            None
        }
    }

    pub fn as_vec2(self) -> Option<[f32; 2]> {
        if self.tag == VfValueTag::Vec2 {
            Some(unsafe { self.payload.vec2 })
        } else {
            None
        }
    }

    pub fn as_vec3(self) -> Option<[f32; 3]> {
        if self.tag == VfValueTag::Vec3 {
            Some(unsafe { self.payload.vec3 })
        } else {
            None
        }
    }

    /// # Safety
    /// Pointer must be valid for the current `process` call.
    pub unsafe fn as_unified(&self) -> Option<&VfUnifiedFrame> {
        if self.tag != VfValueTag::UnifiedFrame {
            return None;
        }
        let p = unsafe { self.payload.unified };
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p })
        }
    }

    /// # Safety
    /// Pointer must be valid and unique for the current `process` call.
    pub unsafe fn as_unified_mut(&mut self) -> Option<&mut VfUnifiedFrame> {
        if self.tag != VfValueTag::UnifiedFrame {
            return None;
        }
        let p = unsafe { self.payload.unified };
        if p.is_null() {
            None
        } else {
            Some(unsafe { &mut *p })
        }
    }
}

/// Whether two ports may be connected.
pub fn ports_compatible(
    src_tag: VfValueTag,
    src_schema: Option<&str>,
    dst_tag: VfValueTag,
    dst_schema: Option<&str>,
) -> bool {
    if src_tag != dst_tag {
        return false;
    }
    match src_tag {
        VfValueTag::Bytes => {
            let s = src_schema.unwrap_or("");
            let d = dst_schema.unwrap_or("");
            s.is_empty() || d.is_empty() || s == "*" || d == "*" || s == d
        }
        _ => true,
    }
}
