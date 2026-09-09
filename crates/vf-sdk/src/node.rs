use crate::desc::NodeDescriptor;
use crate::error::Result;
use crate::host::{Host, NodeStatus, ProcessCtx};
use crate::ports::PortIo;

pub trait Node: Send + 'static {
    fn descriptor() -> NodeDescriptor;

    fn create(host: Host, config: &serde_json::Value) -> Result<Self>
    where
        Self: Sized;

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()>;

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        let _ = (key, value);
        Ok(())
    }

    fn state(&self) -> Option<serde_json::Value> {
        None
    }

    fn set_state(&mut self, _state: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok("")
    }
}
