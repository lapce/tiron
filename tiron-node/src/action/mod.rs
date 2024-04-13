mod command;
mod copy;
pub mod data;
mod file;
mod git;
mod package;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use crossbeam_channel::Sender;
use itertools::Itertools;
use rcl::{error::Error, runtime::Value, source::Span};
use tiron_common::action::{ActionId, ActionMessage};

pub trait Action {
    /// name of the action
    fn name(&self) -> String;

    fn doc(&self) -> ActionDoc;

    fn input(&self, cwd: &std::path::Path, params: ActionParams) -> Result<Vec<u8>, Error>;

    fn execute(
        &self,
        id: ActionId,
        input: &[u8],
        tx: &Sender<ActionMessage>,
    ) -> anyhow::Result<String>;
}

pub enum ActionParamBaseType {
    String,
}

impl ActionParamBaseType {
    fn parse_value(&self, value: &hcl::Value) -> Option<ActionParamBaseValue> {
        match self {
            ActionParamBaseType::String => {
                if let hcl::Value::String(s) = value {
                    return Some(ActionParamBaseValue::String(s.to_string()));
                }
            }
        }
        None
    }

    fn parse(&self, value: &Value) -> Option<ActionParamBaseValue> {
        match self {
            ActionParamBaseType::String => {
                if let Value::String(s, _) = value {
                    return Some(ActionParamBaseValue::String(s.to_string()));
                }
            }
        }
        None
    }
}

impl Display for ActionParamBaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionParamBaseType::String => f.write_str("String"),
        }
    }
}

pub enum ActionParamType {
    String,
    Bool,
    List(ActionParamBaseType),
    Enum(Vec<ActionParamBaseValue>),
}

impl ActionParamType {
    fn parse_attr(&self, value: &hcl::Value) -> Option<ActionParamValue> {
        match self {
            ActionParamType::String => {
                if let hcl::Value::String(s) = value {
                    return Some(ActionParamValue::String(s.to_string(), None));
                }
            }
            ActionParamType::Bool => {
                if let hcl::Value::Bool(v) = value {
                    return Some(ActionParamValue::Bool(*v));
                }
            }
            ActionParamType::List(base) => {
                if let hcl::Value::Array(v) = value {
                    let mut items = Vec::new();
                    for v in v.iter() {
                        let base = base.parse_value(v)?;
                        items.push(base);
                    }
                    return Some(ActionParamValue::List(items));
                }
            }
            ActionParamType::Enum(options) => {
                for option in options {
                    if option.match_value_new(value) {
                        return Some(ActionParamValue::Base(option.clone()));
                    }
                }
            }
        }

        None
    }

    fn parse(&self, value: &Value) -> Option<ActionParamValue> {
        match self {
            ActionParamType::String => {
                if let Value::String(s, span) = value {
                    return Some(ActionParamValue::String(s.to_string(), *span));
                }
            }
            ActionParamType::Bool => {
                if let Value::Bool(v) = value {
                    return Some(ActionParamValue::Bool(*v));
                }
            }
            ActionParamType::List(base) => {
                if let Value::List(v) = value {
                    let mut items = Vec::new();
                    for v in v.iter() {
                        let base = base.parse(v)?;
                        items.push(base);
                    }
                    return Some(ActionParamValue::List(items));
                }
            }
            ActionParamType::Enum(options) => {
                for option in options {
                    if option.match_value(value) {
                        return Some(ActionParamValue::Base(option.clone()));
                    }
                }
            }
        }
        None
    }
}

impl Display for ActionParamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionParamType::String => f.write_str("String"),
            ActionParamType::Bool => f.write_str("Boolean"),
            ActionParamType::List(t) => f.write_str(&format!("List of {t}")),
            ActionParamType::Enum(t) => f.write_str(&format!(
                "Enum of {}",
                t.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

pub struct ActionParamDoc {
    pub name: String,
    pub required: bool,
    pub type_: Vec<ActionParamType>,
    pub description: String,
}

impl ActionParamDoc {
    fn parse_attrs(
        &self,
        attrs: &HashMap<String, hcl::Value>,
    ) -> Result<Option<ActionParamValue>, Error> {
        let param = attrs.get(&self.name);

        if let Some(param) = param {
            for type_ in &self.type_ {
                if let Some(value) = type_.parse_attr(param) {
                    return Ok(Some(value));
                }
            }
            return Error::new(format!(
                "{} type should be {}",
                self.name,
                self.type_.iter().map(|t| t.to_string()).join(" or ")
            ))
            .err();
        }

        if self.required {
            return Error::new(format!("can't find {}", self.name,)).err();
        }

        Ok(None)
    }

    pub fn parse_param(
        &self,
        dict: &BTreeMap<Value, Value>,
        dict_span: Option<Span>,
    ) -> Result<Option<ActionParamValue>, Error> {
        let param = dict.get(&Value::String(self.name.clone().into(), None));
        if let Some(param) = param {
            for type_ in &self.type_ {
                if let Some(value) = type_.parse(param) {
                    return Ok(Some(value));
                }
            }
            return Error::new(format!(
                "{} type should be {}",
                self.name,
                self.type_.iter().map(|t| t.to_string()).join(" or ")
            ))
            .with_origin(*param.span())
            .err();
        }

        if self.required {
            return Error::new(format!("can't find {}", self.name,))
                .with_origin(dict_span)
                .err();
        }

        Ok(None)
    }
}

pub struct ActionDoc {
    pub description: String,
    pub params: Vec<ActionParamDoc>,
}

impl ActionDoc {
    pub fn parse_attrs(&self, attrs: &HashMap<String, hcl::Value>) -> Result<ActionParams, Error> {
        let mut values = Vec::new();
        for param in &self.params {
            let value = param.parse_attrs(attrs)?;
            values.push(value);
        }

        Ok(ActionParams { span: None, values })
    }

    pub fn parse_params(&self, params: Option<&Value>) -> Result<ActionParams, Error> {
        let Some(value) = params else {
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = value else {
            return Error::new("params should be a Dict")
                .with_origin(*value.span())
                .err();
        };

        let mut values = Vec::new();
        for param in &self.params {
            let value = param.parse_param(dict, *dict_span)?;
            values.push(value);
        }

        Ok(ActionParams {
            span: *dict_span,
            values,
        })
    }
}

pub struct ActionParams {
    pub span: Option<Span>,
    pub values: Vec<Option<ActionParamValue>>,
}

impl ActionParams {
    pub fn expect_string(&self, i: usize) -> &str {
        self.values[i].as_ref().unwrap().expect_string()
    }

    pub fn expect_string_with_span(&self, i: usize) -> (&str, &Option<Span>) {
        self.values[i].as_ref().unwrap().expect_string_with_span()
    }

    pub fn base(&self, i: usize) -> Option<&ActionParamBaseValue> {
        self.values[i].as_ref().map(|v| v.expect_base())
    }

    pub fn expect_base(&self, i: usize) -> &ActionParamBaseValue {
        self.values[i].as_ref().unwrap().expect_base()
    }

    pub fn list(&self, i: usize) -> Option<&[ActionParamBaseValue]> {
        self.values[i].as_ref().map(|v| v.expect_list())
    }
}

pub enum ActionParamValue {
    String(String, Option<Span>),
    Bool(bool),
    List(Vec<ActionParamBaseValue>),
    Base(ActionParamBaseValue),
}

impl ActionParamValue {
    pub fn string(&self) -> Option<&str> {
        if let ActionParamValue::String(s, _) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn string_with_span(&self) -> Option<(&str, &Option<Span>)> {
        if let ActionParamValue::String(s, span) = self {
            Some((s, span))
        } else {
            None
        }
    }

    pub fn list(&self) -> Option<&[ActionParamBaseValue]> {
        if let ActionParamValue::List(l) = self {
            Some(l)
        } else {
            None
        }
    }

    pub fn base(&self) -> Option<&ActionParamBaseValue> {
        if let ActionParamValue::Base(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn expect_string(&self) -> &str {
        self.string().unwrap()
    }

    pub fn expect_string_with_span(&self) -> (&str, &Option<Span>) {
        self.string_with_span().unwrap()
    }

    pub fn expect_list(&self) -> &[ActionParamBaseValue] {
        self.list().unwrap()
    }

    pub fn expect_base(&self) -> &ActionParamBaseValue {
        self.base().unwrap()
    }
}

#[derive(Clone)]
pub enum ActionParamBaseValue {
    String(String),
}

impl ActionParamBaseValue {
    fn match_value_new(&self, value: &hcl::Value) -> bool {
        match self {
            ActionParamBaseValue::String(base) => {
                if let hcl::Value::String(s) = value {
                    return base == &s.to_string();
                }
            }
        }

        false
    }

    fn match_value(&self, value: &Value) -> bool {
        match self {
            ActionParamBaseValue::String(base) => {
                if let Value::String(s, _) = value {
                    return base == &s.to_string();
                }
            }
        }

        false
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            ActionParamBaseValue::String(s) => Some(s),
        }
    }

    pub fn expect_string(&self) -> &str {
        self.string().unwrap()
    }
}

impl Display for ActionParamBaseValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionParamBaseValue::String(s) => f.write_str(&format!("\"{s}\"")),
        }
    }
}
