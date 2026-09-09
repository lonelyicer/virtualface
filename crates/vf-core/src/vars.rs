use serde::{Deserialize, Serialize};
use vf_abi::VfValueTag;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VarType {
    Float,
    Int,
    Bool,
    String,
}

impl VarType {
    pub const ALL: [VarType; 4] = [VarType::Float, VarType::Int, VarType::Bool, VarType::String];

    pub fn label(self) -> &'static str {
        match self {
            Self::Float => "Float",
            Self::Int => "Int",
            Self::Bool => "Bool",
            Self::String => "String",
        }
    }

    pub fn tag(self) -> VfValueTag {
        match self {
            Self::Float => VfValueTag::Float,
            Self::Int => VfValueTag::Int,
            Self::Bool => VfValueTag::Bool,
            Self::String => VfValueTag::Bytes,
        }
    }

    pub fn default_value(self) -> serde_json::Value {
        match self {
            Self::Float => serde_json::json!(0.0),
            Self::Int => serde_json::json!(0),
            Self::Bool => serde_json::json!(false),
            Self::String => serde_json::json!(""),
        }
    }

    pub fn get_type_id(self) -> &'static str {
        match self {
            Self::Float => "vf.get.float",
            Self::Int => "vf.get.int",
            Self::Bool => "vf.get.bool",
            Self::String => "vf.get.string",
        }
    }

    pub fn set_type_id(self) -> &'static str {
        match self {
            Self::Float => "vf.set.float",
            Self::Int => "vf.set.int",
            Self::Bool => "vf.set.bool",
            Self::String => "vf.set.string",
        }
    }

    pub fn from_type_id(type_id: &str) -> Option<(bool, VarType)> {
        match type_id {
            "vf.get.float" => Some((true, Self::Float)),
            "vf.get.int" => Some((true, Self::Int)),
            "vf.get.bool" => Some((true, Self::Bool)),
            "vf.get.string" => Some((true, Self::String)),
            "vf.set.float" => Some((false, Self::Float)),
            "vf.set.int" => Some((false, Self::Int)),
            "vf.set.bool" => Some((false, Self::Bool)),
            "vf.set.string" => Some((false, Self::String)),
            _ => None,
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Float => 0x22c55e,
            Self::Int => 0x84cc16,
            Self::Bool => 0xef4444,
            Self::String => 0x38bdf8,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphVar {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: VarType,
    pub value: serde_json::Value,
}

impl GraphVar {
    pub fn new(name: impl Into<String>, ty: VarType) -> Self {
        Self {
            name: name.into(),
            ty,
            value: ty.default_value(),
        }
    }
}

pub fn unique_var_name(existing: &[GraphVar], base: &str) -> String {
    let base = sanitize_var_name(base);
    if !existing.iter().any(|v| v.name == base) {
        return base;
    }
    for i in 1..10_000 {
        let candidate = format!("{base}_{i}");
        if !existing.iter().any(|v| v.name == candidate) {
            return candidate;
        }
    }
    format!("{base}_x")
}

pub fn sanitize_var_name(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if c.is_whitespace() || c == '-' {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    if out.is_empty() {
        "NewVar".into()
    } else if out.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
        format!("V_{out}")
    } else {
        out
    }
}
