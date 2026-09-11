use vf_abi::{UnifiedExpression, VF_VALID_EXPR, VF_VALID_EYE, VF_VALID_HEAD, VfUnifiedFrame};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, PortDesc, PortIo, ProcessCtx, Result,
};

/// Merge eye / expression / head from up to three UnifiedFrame sources.
///
/// Each input pin keeps only its own channel so a full-frame source (eye +
/// shapes + head) can be wired to several pins without stacking the extra
/// slices onto the output.
pub struct MergeUnified;

impl Node for MergeUnified {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("unified.merge", "Merge Unified", Category::Process)
            .input(PortDesc::unified("eye"))
            .input(PortDesc::unified("expression"))
            .input(PortDesc::unified("head"))
            .output(PortDesc::unified("out"))
    }

    fn create(_host: Host, _config: &serde_json::Value) -> Result<Self> {
        Ok(Self)
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let out = merge_channels(
            io.input_unified_opt(0),
            io.input_unified_opt(1),
            io.input_unified_opt(2),
        );
        *io.output_unified_mut(0)? = out;
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok("merge eye / expr / head")
    }
}

fn merge_channels(
    eye: Option<&VfUnifiedFrame>,
    expression: Option<&VfUnifiedFrame>,
    head: Option<&VfUnifiedFrame>,
) -> VfUnifiedFrame {
    let mut out = VfUnifiedFrame::default();
    if let Some(src) = eye {
        take_eye(src, &mut out);
    }
    if let Some(src) = expression {
        take_expression(src, &mut out);
    }
    if let Some(src) = head {
        take_head(src, &mut out);
    }
    out
}

fn take_eye(src: &VfUnifiedFrame, out: &mut VfUnifiedFrame) {
    out.eye = src.eye;
    out.valid |= src.valid & VF_VALID_EYE;
    out.timestamp_us = out.timestamp_us.max(src.timestamp_us);
    for expr in UnifiedExpression::ALL {
        if expr.is_eye() {
            out.shapes[expr.index()] = src.shapes[expr.index()];
        }
    }
}

fn take_expression(src: &VfUnifiedFrame, out: &mut VfUnifiedFrame) {
    out.valid |= src.valid & VF_VALID_EXPR;
    out.timestamp_us = out.timestamp_us.max(src.timestamp_us);
    for expr in UnifiedExpression::ALL {
        if !expr.is_eye() {
            out.shapes[expr.index()] = src.shapes[expr.index()];
        }
    }
}

fn take_head(src: &VfUnifiedFrame, out: &mut VfUnifiedFrame) {
    out.head = src.head;
    out.valid |= src.valid & VF_VALID_HEAD;
    out.timestamp_us = out.timestamp_us.max(src.timestamp_us);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vf_abi::{VfEyeData, VfEyeSample, VfHeadData};

    fn full_frame(marker: f32, ts: u64) -> VfUnifiedFrame {
        let mut f = VfUnifiedFrame::default();
        f.timestamp_us = ts;
        f.valid = VF_VALID_EYE | VF_VALID_EXPR | VF_VALID_HEAD;
        f.eye = VfEyeData {
            left: VfEyeSample {
                gaze: [marker, marker + 0.1],
                openness: marker,
                pupil_mm: marker,
            },
            right: VfEyeSample {
                gaze: [marker + 0.2, marker + 0.3],
                openness: marker + 0.05,
                pupil_mm: marker + 0.05,
            },
        };
        f.head = VfHeadData {
            yaw: marker,
            pitch: marker + 1.0,
            roll: marker + 2.0,
            pos: [marker, marker + 1.0, marker + 2.0],
        };
        for expr in UnifiedExpression::ALL {
            f.shapes[expr.index()] = marker + expr.index() as f32 * 0.001;
        }
        f
    }

    #[test]
    fn pins_keep_only_their_channel() {
        let eye_src = full_frame(1.0, 10);
        let expr_src = full_frame(2.0, 20);
        let head_src = full_frame(3.0, 30);

        let out = merge_channels(Some(&eye_src), Some(&expr_src), Some(&head_src));

        assert_eq!(out.eye.left.openness, 1.0);
        assert_eq!(out.head.yaw, 3.0);
        assert_eq!(
            out.shapes[UnifiedExpression::EyeSquintLeft.index()],
            1.0 + UnifiedExpression::EyeSquintLeft.index() as f32 * 0.001
        );
        assert_eq!(
            out.shapes[UnifiedExpression::JawOpen.index()],
            2.0 + UnifiedExpression::JawOpen.index() as f32 * 0.001
        );
        assert_eq!(out.valid, VF_VALID_EYE | VF_VALID_EXPR | VF_VALID_HEAD);
        assert_eq!(out.timestamp_us, 30);
    }

    #[test]
    fn expression_pin_does_not_leak_eye_or_head() {
        let src = full_frame(4.0, 40);
        let out = merge_channels(None, Some(&src), None);

        assert_eq!(out.eye.left.openness, 0.0);
        assert_eq!(out.head.yaw, 0.0);
        assert_eq!(out.shapes[UnifiedExpression::EyeWideRight.index()], 0.0);
        assert_eq!(
            out.shapes[UnifiedExpression::JawOpen.index()],
            4.0 + UnifiedExpression::JawOpen.index() as f32 * 0.001
        );
        assert_eq!(out.valid, VF_VALID_EXPR);
    }

    #[test]
    fn eye_pin_does_not_leak_mouth_or_head() {
        let src = full_frame(5.0, 50);
        let out = merge_channels(Some(&src), None, None);

        assert_eq!(out.eye.left.openness, 5.0);
        assert_eq!(out.head.yaw, 0.0);
        assert_eq!(out.shapes[UnifiedExpression::JawOpen.index()], 0.0);
        assert!(out.shapes[UnifiedExpression::EyeSquintRight.index()] > 0.0);
        assert_eq!(out.valid, VF_VALID_EYE);
    }
}
