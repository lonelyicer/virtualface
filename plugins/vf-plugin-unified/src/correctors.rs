//! Unified correctors (MouthClosed clamp, LipSuck limiter, optional eyelid blend).

use vf_abi::UnifiedExpression as U;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_bool, param_f32,
};

pub struct Correctors {
    mouth_closed_fix: bool,
    lip_suck_fix: bool,
    eye_lid_blend: f32,
    eye_look_sym: bool,
}

impl Node for Correctors {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new(
            "unified.correctors",
            "Unified Correctors",
            Category::Process,
        )
        .input(PortDesc::unified("in"))
        .output(PortDesc::unified("out"))
        .param(ParamDef::boolean(
            "mouth_closed_fix",
            "MouthClosed/JawOpen Clamp",
            true,
        ))
        .param(ParamDef::boolean("lip_suck_fix", "LipSuck Limiter", true))
        .param(ParamDef::float(
            "eye_lid_blend",
            "EyeLid Blend",
            0.0,
            0.0,
            1.0,
            0.01,
        ))
        .param(ParamDef::boolean(
            "eye_look_symmetrize",
            "EyeLook Symmetrize",
            false,
        ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            mouth_closed_fix: param_bool(config, "mouth_closed_fix", true),
            lip_suck_fix: param_bool(config, "lip_suck_fix", true),
            eye_lid_blend: param_f32(config, "eye_lid_blend", 0.0),
            eye_look_sym: param_bool(config, "eye_look_symmetrize", false),
        })
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let mut f = *io.input_unified(0)?;
        if self.mouth_closed_fix {
            let i = U::MouthClosed.index();
            f.shapes[i] = f.shapes[i].min(f.shapes[U::JawOpen.index()]);
        }
        if self.lip_suck_fix {
            f.shapes[U::LipSuckLowerLeft.index()] *= 1.0 - f.shapes[U::MouthLowerDownLeft.index()];
            f.shapes[U::LipSuckLowerRight.index()] *=
                1.0 - f.shapes[U::MouthLowerDownRight.index()];
            f.shapes[U::LipSuckUpperLeft.index()] *= 1.0 - f.shapes[U::MouthUpperUpLeft.index()];
            f.shapes[U::LipSuckUpperRight.index()] *= 1.0 - f.shapes[U::MouthUpperUpRight.index()];
        }
        if self.eye_lid_blend > 0.0 {
            let k = self.eye_lid_blend * 0.5;
            let blend = |a: f32, b: f32| (a * (1.0 - k) + b * k).clamp(0.0, 1.0);
            let (l, r) = (f.eye.left.openness, f.eye.right.openness);
            f.eye.left.openness = blend(l, r);
            f.eye.right.openness = blend(r, f.eye.left.openness);
        }
        if self.eye_look_sym {
            let y = (f.eye.left.gaze[1] + f.eye.right.gaze[1]) * 0.5;
            f.eye.left.gaze[1] = y;
            f.eye.right.gaze[1] = y;
        }
        *io.output_unified_mut(0)? = f;
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "mouth_closed_fix" => self.mouth_closed_fix = vf_sdk::json_bool(value, true),
            "lip_suck_fix" => self.lip_suck_fix = vf_sdk::json_bool(value, true),
            "eye_lid_blend" => self.eye_lid_blend = vf_sdk::json_f64(value, 0.0) as f32,
            "eye_look_symmetrize" => self.eye_look_sym = vf_sdk::json_bool(value, false),
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok("correctors")
    }
}
