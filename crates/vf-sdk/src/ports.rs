use crate::error::{Result, SdkError};
use vf_abi::{VfUnifiedFrame, VfValue, VfValueTag};

pub struct PortIo<'a> {
    pub inputs: &'a [VfValue],
    pub outputs: &'a mut [VfValue],
}

impl<'a> PortIo<'a> {
    pub fn input_float(&self, i: usize) -> Option<f32> {
        self.inputs.get(i).and_then(|v| v.as_float())
    }

    pub fn input_bool(&self, i: usize) -> Option<bool> {
        self.inputs.get(i).and_then(|v| v.as_bool())
    }

    pub fn input_vec2(&self, i: usize) -> Option<[f32; 2]> {
        self.inputs.get(i).and_then(|v| v.as_vec2())
    }

    pub fn input_unified(&self, i: usize) -> Result<&VfUnifiedFrame> {
        let v = self
            .inputs
            .get(i)
            .ok_or_else(|| SdkError::other("missing unified input"))?;
        unsafe {
            v.as_unified()
                .ok_or_else(|| SdkError::other("input is not a UnifiedFrame"))
        }
    }

    pub fn input_unified_opt(&self, i: usize) -> Option<&VfUnifiedFrame> {
        self.inputs.get(i).and_then(|v| unsafe { v.as_unified() })
    }

    pub fn input_blendshapes(&self, i: usize) -> Result<&[f32]> {
        let v = self
            .inputs
            .get(i)
            .ok_or_else(|| SdkError::other("missing blendshape input"))?;
        if v.tag != VfValueTag::Blendshapes {
            return Err(SdkError::other("input is not blendshapes"));
        }
        let buf = unsafe { &v.payload.blendshapes };
        Ok(unsafe { buf.as_slice() })
    }

    pub fn output_float(&mut self, i: usize, value: f32) -> Result<()> {
        let slot = self
            .outputs
            .get_mut(i)
            .ok_or_else(|| SdkError::other("missing float output"))?;
        slot.tag = VfValueTag::Float;
        slot.payload.float = value;
        Ok(())
    }

    pub fn output_bool(&mut self, i: usize, value: bool) -> Result<()> {
        let slot = self
            .outputs
            .get_mut(i)
            .ok_or_else(|| SdkError::other("missing bool output"))?;
        slot.tag = VfValueTag::Bool;
        slot.payload.boolean = u8::from(value);
        Ok(())
    }

    pub fn output_unified_mut(&mut self, i: usize) -> Result<&mut VfUnifiedFrame> {
        let slot = self
            .outputs
            .get_mut(i)
            .ok_or_else(|| SdkError::other("missing unified output"))?;
        unsafe {
            slot.as_unified_mut()
                .ok_or_else(|| SdkError::other("output is not a UnifiedFrame"))
        }
    }

    pub fn output_blendshapes_mut(&mut self, i: usize) -> Result<&mut [f32]> {
        let slot = self
            .outputs
            .get_mut(i)
            .ok_or_else(|| SdkError::other("missing blendshape output"))?;
        if slot.tag != VfValueTag::Blendshapes {
            return Err(SdkError::other("output is not blendshapes"));
        }
        let p = unsafe { slot.payload.blendshapes };
        if p.ptr.is_null() {
            return Ok(&mut []);
        }
        Ok(unsafe { core::slice::from_raw_parts_mut(p.ptr, p.len as usize) })
    }
}
