use serde::{Deserialize, Serialize};
use vf_abi::{VfCategory, VfValueTag};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Input,
    Process,
    Output,
    Utility,
}

impl From<Category> for VfCategory {
    fn from(c: Category) -> Self {
        match c {
            Category::Input => VfCategory::Input,
            Category::Process => VfCategory::Process,
            Category::Output => VfCategory::Output,
            Category::Utility => VfCategory::Utility,
        }
    }
}

impl From<VfCategory> for Category {
    fn from(c: VfCategory) -> Self {
        match c {
            VfCategory::Input => Category::Input,
            VfCategory::Process => Category::Process,
            VfCategory::Output => Category::Output,
            VfCategory::Utility => Category::Utility,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PortDesc {
    pub name: &'static str,
    pub ty: VfValueTag,
    pub schema: &'static str,
    pub capacity: u32,
}

impl PortDesc {
    pub const fn unified(name: &'static str) -> Self {
        Self {
            name,
            ty: VfValueTag::UnifiedFrame,
            schema: "",
            capacity: 0,
        }
    }

    pub const fn blendshapes(name: &'static str, schema: &'static str, capacity: u32) -> Self {
        Self {
            name,
            ty: VfValueTag::Blendshapes,
            schema,
            capacity,
        }
    }

    pub const fn float(name: &'static str) -> Self {
        Self {
            name,
            ty: VfValueTag::Float,
            schema: "",
            capacity: 0,
        }
    }

    pub const fn boolean(name: &'static str) -> Self {
        Self {
            name,
            ty: VfValueTag::Bool,
            schema: "",
            capacity: 0,
        }
    }

    pub const fn vec2(name: &'static str) -> Self {
        Self {
            name,
            ty: VfValueTag::Vec2,
            schema: "",
            capacity: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParamDef {
    pub key: String,
    pub kind: ParamKind,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    Float,
    Int,
    Bool,
    Enum,
    String,
    Button,
}

impl ParamDef {
    pub fn float(key: &str, label: &str, default: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::Float,
            label: label.into(),
            min: Some(min),
            max: Some(max),
            step: Some(step),
            default: Some(serde_json::json!(default)),
            options: vec![],
        }
    }

    pub fn int(key: &str, label: &str, default: i64, min: i64, max: i64) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::Int,
            label: label.into(),
            min: Some(min as f64),
            max: Some(max as f64),
            step: Some(1.0),
            default: Some(serde_json::json!(default)),
            options: vec![],
        }
    }

    pub fn boolean(key: &str, label: &str, default: bool) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::Bool,
            label: label.into(),
            min: None,
            max: None,
            step: None,
            default: Some(serde_json::json!(default)),
            options: vec![],
        }
    }

    pub fn string(key: &str, label: &str, default: &str) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::String,
            label: label.into(),
            min: None,
            max: None,
            step: None,
            default: Some(serde_json::json!(default)),
            options: vec![],
        }
    }

    pub fn enum_str(key: &str, label: &str, default: &str, options: &[&str]) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::Enum,
            label: label.into(),
            min: None,
            max: None,
            step: None,
            default: Some(serde_json::json!(default)),
            options: options.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    pub fn button(key: &str, label: &str) -> Self {
        Self {
            key: key.into(),
            kind: ParamKind::Button,
            label: label.into(),
            min: None,
            max: None,
            step: None,
            default: None,
            options: vec![],
        }
    }
}

pub fn defaults_from_schema(schema: &[ParamDef]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for p in schema {
        if let Some(d) = &p.default {
            map.insert(p.key.clone(), d.clone());
        }
    }
    serde_json::Value::Object(map)
}

#[derive(Clone, Debug)]
pub struct NodeDescriptor {
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub category: Category,
    pub inputs: Vec<PortDesc>,
    pub outputs: Vec<PortDesc>,
    pub params: Vec<ParamDef>,
    pub flags: u32,
}

impl NodeDescriptor {
    pub fn new(type_id: &'static str, display_name: &'static str, category: Category) -> Self {
        Self {
            type_id,
            display_name,
            category,
            inputs: vec![],
            outputs: vec![],
            params: vec![],
            flags: 0,
        }
    }

    pub fn input(mut self, port: PortDesc) -> Self {
        self.inputs.push(port);
        self
    }

    pub fn output(mut self, port: PortDesc) -> Self {
        self.outputs.push(port);
        self
    }

    pub fn param(mut self, p: ParamDef) -> Self {
        self.params.push(p);
        self
    }

    pub fn source(mut self) -> Self {
        self.flags |= vf_abi::VF_NODE_IS_SOURCE;
        self
    }
}
