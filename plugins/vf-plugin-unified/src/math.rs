use vf_abi::UnifiedExpression;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_f32, param_string,
};

pub struct ShapeMath {
    shape: UnifiedExpression,
    gain: f32,
    offset: f32,
    clamp_min: f32,
    clamp_max: f32,
}

impl ShapeMath {
    fn parse_shape(name: &str) -> UnifiedExpression {
        UnifiedExpression::from_name(name).unwrap_or(UnifiedExpression::JawOpen)
    }
}

impl Node for ShapeMath {
    fn descriptor() -> NodeDescriptor {
        let names: Vec<&str> = UnifiedExpression::ALL.iter().map(|e| e.name()).collect();
        NodeDescriptor::new("unified.shape_math", "Shape Math", Category::Process)
            .input(PortDesc::unified("in"))
            .output(PortDesc::unified("out"))
            .param(ParamDef::enum_str("shape", "Shape", "JawOpen", &names))
            .param(ParamDef::float("gain", "Gain", 1.0, 0.0, 4.0, 0.01))
            .param(ParamDef::float("offset", "Offset", 0.0, -1.0, 1.0, 0.01))
            .param(ParamDef::float(
                "clamp_min",
                "Clamp Min",
                0.0,
                -1.0,
                1.0,
                0.01,
            ))
            .param(ParamDef::float(
                "clamp_max",
                "Clamp Max",
                1.0,
                0.0,
                2.0,
                0.01,
            ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            shape: Self::parse_shape(&param_string(config, "shape", "JawOpen")),
            gain: param_f32(config, "gain", 1.0),
            offset: param_f32(config, "offset", 0.0),
            clamp_min: param_f32(config, "clamp_min", 0.0),
            clamp_max: param_f32(config, "clamp_max", 1.0),
        })
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let mut f = *io.input_unified(0)?;
        let i = self.shape.index();
        let v = (f.shapes[i] * self.gain + self.offset).clamp(self.clamp_min, self.clamp_max);
        f.shapes[i] = v;
        *io.output_unified_mut(0)? = f;
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "shape" => {
                if let Some(s) = value.as_str() {
                    self.shape = Self::parse_shape(s);
                }
            }
            "gain" => self.gain = vf_sdk::json_f64(value, 1.0) as f32,
            "offset" => self.offset = vf_sdk::json_f64(value, 0.0) as f32,
            "clamp_min" => self.clamp_min = vf_sdk::json_f64(value, 0.0) as f32,
            "clamp_max" => self.clamp_max = vf_sdk::json_f64(value, 1.0) as f32,
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok(format!(
            "{} ×{:.2} {:+.2}",
            self.shape.name(),
            self.gain,
            self.offset
        ))
    }
}
