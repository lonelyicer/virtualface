use vf_abi::{VF_VALID_EXPR, VF_VALID_EYE, VF_VALID_HEAD, VfUnifiedFrame};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, PortDesc, PortIo, ProcessCtx, Result,
};

/// Merge eye / expression / head from up to three UnifiedFrame sources.
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
        let mut out = VfUnifiedFrame::default();
        if let Some(e) = io.input_unified_opt(0) {
            out.eye = e.eye;
            out.valid |= e.valid & VF_VALID_EYE;
            out.timestamp_us = out.timestamp_us.max(e.timestamp_us);
        }
        if let Some(e) = io.input_unified_opt(1) {
            out.shapes = e.shapes;
            out.valid |= e.valid & VF_VALID_EXPR;
            out.timestamp_us = out.timestamp_us.max(e.timestamp_us);
            if io.input_unified_opt(0).is_none() {
                out.eye = e.eye;
                out.valid |= e.valid & VF_VALID_EYE;
            }
        }
        if let Some(e) = io.input_unified_opt(2) {
            out.head = e.head;
            out.valid |= e.valid & VF_VALID_HEAD;
            out.timestamp_us = out.timestamp_us.max(e.timestamp_us);
        }
        *io.output_unified_mut(0)? = out;
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok("merge eye / expr / head")
    }
}
