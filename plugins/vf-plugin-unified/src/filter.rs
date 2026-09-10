//! One Euro filter using real dt (VRCFT `Filter.cs` used a fixed 10 Hz).

use vf_abi::{VF_UNIFIED_SHAPE_COUNT, VfUnifiedFrame};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_f32,
};

struct Euro {
    x_prev: f32,
    dx_prev: f32,
    primed: bool,
}

impl Euro {
    fn new() -> Self {
        Self {
            x_prev: 0.0,
            dx_prev: 0.0,
            primed: false,
        }
    }

    fn filter(&mut self, x: f32, dt: f32, min_cutoff: f32, beta: f32, d_cutoff: f32) -> f32 {
        if !x.is_finite() {
            return 0.0;
        }
        if !self.primed {
            self.x_prev = x;
            self.dx_prev = 0.0;
            self.primed = true;
            return x;
        }
        let hz = (1.0 / dt.max(1e-4)).min(1000.0);
        let dx = (x - self.x_prev) * hz;
        let edx = lowpass(&mut self.dx_prev, dx, alpha(hz, d_cutoff));
        let cutoff = min_cutoff + beta * edx.abs();
        lowpass(&mut self.x_prev, x, alpha(hz, cutoff))
    }
}

fn alpha(hz: f32, cutoff: f32) -> f32 {
    let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(1e-6));
    let te = 1.0 / hz.max(1e-6);
    1.0 / (1.0 + tau / te)
}

fn lowpass(hat_prev: &mut f32, x: f32, a: f32) -> f32 {
    let hat = a * x + (1.0 - a) * *hat_prev;
    *hat_prev = hat;
    hat
}

pub struct OneEuroFilter {
    min_cutoff: f32,
    beta: f32,
    d_cutoff: f32,
    shapes: Vec<Euro>,
    gaze_lx: Euro,
    gaze_ly: Euro,
    gaze_rx: Euro,
    gaze_ry: Euro,
    pupil_l: Euro,
    pupil_r: Euro,
    open_l: Euro,
    open_r: Euro,
    head: [Euro; 6],
}

impl OneEuroFilter {
    fn new(min_cutoff: f32, beta: f32, d_cutoff: f32) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff,
            shapes: (0..VF_UNIFIED_SHAPE_COUNT).map(|_| Euro::new()).collect(),
            gaze_lx: Euro::new(),
            gaze_ly: Euro::new(),
            gaze_rx: Euro::new(),
            gaze_ry: Euro::new(),
            pupil_l: Euro::new(),
            pupil_r: Euro::new(),
            open_l: Euro::new(),
            open_r: Euro::new(),
            head: std::array::from_fn(|_| Euro::new()),
        }
    }

    pub fn apply(&mut self, mut f: VfUnifiedFrame, dt: f32) -> VfUnifiedFrame {
        let (mc, b, dc) = (self.min_cutoff, self.beta, self.d_cutoff);
        for i in 0..VF_UNIFIED_SHAPE_COUNT {
            f.shapes[i] = self.shapes[i].filter(f.shapes[i], dt, mc, b, dc);
        }
        f.eye.left.openness = self.open_l.filter(f.eye.left.openness, dt, mc, b, dc);
        f.eye.right.openness = self.open_r.filter(f.eye.right.openness, dt, mc, b, dc);
        f.eye.left.pupil_mm = self.pupil_l.filter(f.eye.left.pupil_mm, dt, mc, b, dc);
        f.eye.right.pupil_mm = self.pupil_r.filter(f.eye.right.pupil_mm, dt, mc, b, dc);
        f.eye.left.gaze[0] = self.gaze_lx.filter(f.eye.left.gaze[0], dt, mc, b, dc);
        f.eye.left.gaze[1] = self.gaze_ly.filter(f.eye.left.gaze[1], dt, mc, b, dc);
        f.eye.right.gaze[0] = self.gaze_rx.filter(f.eye.right.gaze[0], dt, mc, b, dc);
        f.eye.right.gaze[1] = self.gaze_ry.filter(f.eye.right.gaze[1], dt, mc, b, dc);
        f.head.yaw = self.head[0].filter(f.head.yaw, dt, mc, b, dc);
        f.head.pitch = self.head[1].filter(f.head.pitch, dt, mc, b, dc);
        f.head.roll = self.head[2].filter(f.head.roll, dt, mc, b, dc);
        f.head.pos[0] = self.head[3].filter(f.head.pos[0], dt, mc, b, dc);
        f.head.pos[1] = self.head[4].filter(f.head.pos[1], dt, mc, b, dc);
        f.head.pos[2] = self.head[5].filter(f.head.pos[2], dt, mc, b, dc);
        f
    }
}

impl Node for OneEuroFilter {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("unified.one_euro", "One Euro Filter", Category::Process)
            .input(PortDesc::unified("in"))
            .output(PortDesc::unified("out"))
            .param(ParamDef::float(
                "min_cutoff",
                "Minimum Cutoff",
                1.0,
                0.0,
                2.0,
                0.01,
            ))
            .param(ParamDef::float("beta", "Beta", 0.5, 0.0, 2.0, 0.01))
            .param(ParamDef::float(
                "d_cutoff",
                "Derivative Cutoff",
                0.1,
                0.0,
                2.0,
                0.01,
            ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self::new(
            param_f32(config, "min_cutoff", 1.0),
            param_f32(config, "beta", 0.5),
            param_f32(config, "d_cutoff", 0.1),
        ))
    }

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let frame = *io.input_unified(0)?;
        *io.output_unified_mut(0)? = self.apply(frame, ctx.dt_secs());
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "min_cutoff" => self.min_cutoff = vf_sdk::json_f64(value, 1.0) as f32,
            "beta" => self.beta = vf_sdk::json_f64(value, 0.5) as f32,
            "d_cutoff" => self.d_cutoff = vf_sdk::json_f64(value, 0.1) as f32,
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok(format!("beta {:.2}", self.beta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damps_spike() {
        let mut f = OneEuroFilter::new(1.0, 0.5, 0.1);
        let mut frame = VfUnifiedFrame::default();
        frame.shapes[0] = 0.0;
        let a = f.apply(frame, 0.01);
        frame.shapes[0] = 1.0;
        let b = f.apply(frame, 0.01);
        assert!(
            b.shapes[0] < 0.9,
            "spike should be filtered, got {}",
            b.shapes[0]
        );
        assert_eq!(a.shapes[0], 0.0);
    }
}
