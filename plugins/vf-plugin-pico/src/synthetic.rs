use crate::ue::UeMapper;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_f32,
};

/// Generates a looping Unified Expressions signal for tests and UI demos (no hardware).
pub struct SyntheticSource {
    phase: f32,
    hz: f32,
    amplitude: f32,
    mapper: UeMapper,
}

impl Node for SyntheticSource {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("pico.synthetic", "Synthetic Face Source", Category::Input)
            .source()
            .output(PortDesc::unified("unified"))
            .output(PortDesc::float("timeout"))
            .param(ParamDef::float("hz", "Frequency", 0.4, 0.05, 5.0, 0.05))
            .param(ParamDef::float(
                "amplitude",
                "Amplitude",
                0.8,
                0.0,
                1.0,
                0.01,
            ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            phase: 0.0,
            hz: param_f32(config, "hz", 0.4),
            amplitude: param_f32(config, "amplitude", 0.8),
            mapper: UeMapper::default(),
        })
    }

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        self.phase += ctx.dt_secs() * self.hz * std::f32::consts::TAU;
        let s = (self.phase.sin() * 0.5 + 0.5) * self.amplitude;
        let c = (self.phase.cos() * 0.5 + 0.5) * self.amplitude;
        let mut arkit = [0f32; 52];
        arkit[0] = (1.0 - s).clamp(0.0, 1.0) * 0.15;
        arkit[7] = (1.0 - s).clamp(0.0, 1.0) * 0.15;
        arkit[17] = s;
        arkit[23] = c;
        arkit[24] = c;
        arkit[43] = s * 0.5;
        arkit[4] = (self.phase.sin() * 0.2).max(0.0);
        arkit[11] = arkit[4];
        *io.output_unified_mut(0)? = self.mapper.map_arkit(&arkit, ctx.now_us);
        io.output_float(1, 0.0)?;
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "hz" => self.hz = vf_sdk::json_f64(value, self.hz as f64) as f32,
            "amplitude" => self.amplitude = vf_sdk::json_f64(value, self.amplitude as f64) as f32,
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok(format!("synthetic {:.2} Hz", self.hz))
    }
}
