//! Safe plugin authoring SDK wrapping [`vf_abi`].

pub mod desc;
pub mod error;
pub mod export;
pub mod host;
pub mod node;
pub mod ports;

pub use desc::*;
pub use error::{Result, SdkError};
pub use host::{Host, NodeStatus, NodeStatusKind, ProcessCtx};
pub use node::Node;
pub use ports::PortIo;
pub use vf_abi::{
    ARKIT_BLENDSHAPE_COUNT, PICO_BLENDSHAPE_COUNT, SCHEMA_ARKIT52, SCHEMA_PICO72, SCHEMA_VISEMES20,
    UnifiedExpression, UnifiedSimpleExpression, VF_NODE_IS_SOURCE, VF_UNIFIED_SHAPE_COUNT,
    VF_VALID_EXPR, VF_VALID_EYE, VF_VALID_HEAD, VISEME_COUNT, VfUnifiedFrame, VfValueTag,
};

pub fn json_f64(v: &serde_json::Value, default: f64) -> f64 {
    v.as_f64()
        .or_else(|| v.as_i64().map(|i| i as f64))
        .unwrap_or(default)
}

pub fn json_i64(v: &serde_json::Value, default: i64) -> i64 {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .unwrap_or(default)
}

pub fn json_bool(v: &serde_json::Value, default: bool) -> bool {
    v.as_bool().unwrap_or(default)
}

pub fn json_str(v: &serde_json::Value) -> Option<&str> {
    v.as_str()
}

pub fn param_f32(config: &serde_json::Value, key: &str, default: f32) -> f32 {
    config
        .get(key)
        .map(|v| json_f64(v, default as f64) as f32)
        .unwrap_or(default)
}

pub fn param_i64(config: &serde_json::Value, key: &str, default: i64) -> i64 {
    config
        .get(key)
        .map(|v| json_i64(v, default))
        .unwrap_or(default)
}

pub fn param_bool(config: &serde_json::Value, key: &str, default: bool) -> bool {
    config
        .get(key)
        .map(|v| json_bool(v, default))
        .unwrap_or(default)
}

pub fn param_string(config: &serde_json::Value, key: &str, default: &str) -> String {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_schema_roundtrip() {
        let schema = vec![
            ParamDef::int("port", "UDP Port", 29765, 1, 65535),
            ParamDef::float("min_cutoff", "Min Cutoff", 1.0, 0.0, 2.0, 0.01),
            ParamDef::boolean("continuous", "Continuous", false),
        ];
        let json = serde_json::to_string(&schema).unwrap();
        let back: Vec<ParamDef> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].key, "port");
        let defaults = defaults_from_schema(&schema);
        assert_eq!(defaults["port"], 29765);
        assert_eq!(defaults["continuous"], false);
    }

    struct Dummy;

    impl Node for Dummy {
        fn descriptor() -> NodeDescriptor {
            NodeDescriptor::new("test.dummy", "Dummy", Category::Utility)
                .input(PortDesc::float("in"))
                .output(PortDesc::float("out"))
        }
        fn create(_host: Host, _config: &serde_json::Value) -> Result<Self> {
            Ok(Self)
        }
        fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
            if let Some(v) = io.input_float(0) {
                io.output_float(0, v)?;
            }
            Ok(())
        }
    }

    #[test]
    fn export_plugin_symbol_exists() {
        // Ensures the macro expands and the Node vtable is constructed.
        let desc = Dummy::descriptor();
        assert_eq!(desc.type_id, "test.dummy");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);
    }
}
