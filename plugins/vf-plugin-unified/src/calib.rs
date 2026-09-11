//! Rolling-window per-shape calibration, ported from VRCFT `Calibration.cs`.

use serde::{Deserialize, Serialize};
use vf_abi::{UnifiedExpression, VF_UNIFIED_SHAPE_COUNT, VfUnifiedFrame};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_bool, param_f32,
};

const POINTS: usize = 64;
const S_DELTA: f32 = 0.15;

#[derive(Clone, Serialize, Deserialize)]
struct CalibParam {
    name: String,
    data: Vec<f32>,
    rolling: usize,
    fixed: usize,
    progress: f32,
    max: f32,
    current_step: f32,
}

impl CalibParam {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: vec![0.0; POINTS],
            rolling: 0,
            fixed: 0,
            progress: 0.0,
            max: 0.0,
            current_step: f32::NAN,
        }
    }

    fn update(&mut self, current: f32, continuous: bool, dt: f32) {
        let diff = (current - self.current_step).abs();
        if self.current_step.is_nan() || diff >= S_DELTA * dt {
            if self.fixed < POINTS {
                self.fixed += 1;
                self.progress = self.fixed as f32 / POINTS as f32;
            }
            self.data[self.rolling] = current;
            if self.fixed < POINTS || continuous {
                self.rolling = (self.rolling + 1) % POINTS;
                if self.fixed as f32 >= 0.1 * POINTS as f32 {
                    let m = self.data[..self.fixed.min(POINTS)]
                        .iter()
                        .copied()
                        .fold(0.0, f32::max);
                    if m > self.max {
                        self.max = m;
                    }
                }
            }
        }
        self.current_step = (current / (S_DELTA * dt).max(1e-6)).floor() * (S_DELTA * dt).max(1e-6);
    }

    fn apply(&self, current: f32, k: f32) -> f32 {
        if current.is_nan() || self.max == 0.0 {
            return current;
        }
        let confidence = k * self.progress;
        let adjusted = confidence * (current / self.max) + (1.0 - confidence) * current;
        if adjusted.is_nan() || adjusted.is_infinite() {
            current
        } else {
            adjusted
        }
    }
}

pub struct Calibration {
    active_blend: f32,
    continuous: bool,
    shapes: Vec<CalibParam>,
}

impl Calibration {
    fn reset(&mut self) {
        self.shapes = UnifiedExpression::ALL
            .iter()
            .map(|e| CalibParam::new(e.name()))
            .collect();
        while self.shapes.len() < VF_UNIFIED_SHAPE_COUNT {
            self.shapes.push(CalibParam::new("pad"));
        }
    }
}

impl Node for Calibration {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("unified.calibration", "Calibration", Category::Process)
            .input(PortDesc::unified("in"))
            .output(PortDesc::unified("out"))
            .param(ParamDef::float(
                "calibration_blend",
                "Calibration Blend",
                1.0,
                0.0,
                1.0,
                0.01,
            ))
            .param(ParamDef::boolean(
                "continuous",
                "Continuous Calibration",
                false,
            ))
            .param(ParamDef::button("reset", "Reset Calibration"))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        let mut s = Self {
            active_blend: param_f32(config, "calibration_blend", 1.0),
            continuous: param_bool(config, "continuous", false),
            shapes: vec![],
        };
        s.reset();
        Ok(s)
    }

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let mut frame = *io.input_unified(0)?;
        let dt = ctx.dt_secs();
        let n = UnifiedExpression::ALL.len().min(self.shapes.len());
        for i in 0..n {
            self.shapes[i].update(frame.shapes[i], self.continuous, dt);
            frame.shapes[i] = self.shapes[i].apply(frame.shapes[i], self.active_blend);
        }
        *io.output_unified_mut(0)? = frame;
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "calibration_blend" => self.active_blend = vf_sdk::json_f64(value, 1.0) as f32,
            "continuous" => self.continuous = vf_sdk::json_bool(value, false),
            "reset" => self.reset(),
            _ => {}
        }
        Ok(())
    }

    fn state(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.shapes).ok()
    }

    fn set_state(&mut self, state: &serde_json::Value) -> Result<()> {
        if let Ok(s) = serde_json::from_value::<Vec<CalibParam>>(state.clone())
            && s.len() == self.shapes.len()
        {
            self.shapes = s;
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        let avg =
            self.shapes.iter().map(|s| s.progress).sum::<f32>() / self.shapes.len().max(1) as f32;
        NodeStatus::ok(format!("calib {:.0}%", avg * 100.0))
    }
}

pub fn _use_frame(_: &VfUnifiedFrame) {}
