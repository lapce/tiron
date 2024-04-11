mod command;
mod copy;
pub mod data;
mod file;
mod git;
mod package;

use std::{collections::BTreeMap, fmt::Display};

use crossbeam_channel::Sender;
use itertools::Itertools;
use rcl::{error::Error, runtime::Value, source::Span};
use tiron_common::action::{ActionId, ActionMessage};

pub trait Action {
    /// name of the action
    fn name(&self) -> String;

    fn doc(&self) -> ActionDoc;

    fn input(
        &self,
        cwd: &std::path::Path,
        params: Option<&rcl::runtime::Value>,
    ) -> Result<Vec<u8>, Error>;

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
    Boolean,
    List(ActionParamBaseType),
    Enum(Vec<ActionParamBaseValue>),
}

impl ActionParamType {
    fn parse(&self, value: &Value) -> Option<ActionParamValue> {
        match self {
            ActionParamType::String => {
                if let Value::String(s, _) = value {
                    return Some(ActionParamValue::String(s.to_string()));
                }
            }
            ActionParamType::Boolean => {
                if let Value::Bool(v) = value {
                    return Some(ActionParamValue::Boolean(*v));
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
            ActionParamType::Boolean => f.write_str("Boolean"),
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
    pub fn parse_params(
        &self,
        params: Option<&Value>,
    ) -> Result<Vec<Option<ActionParamValue>>, Error> {
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

        Ok(values)
    }
}

pub enum ActionParamValue {
    String(String),
    Boolean(bool),
    List(Vec<ActionParamBaseValue>),
    Base(ActionParamBaseValue),
}

#[derive(Clone)]
pub enum ActionParamBaseValue {
    String(String),
}

impl ActionParamBaseValue {
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
}

impl Display for ActionParamBaseValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionParamBaseValue::String(s) => f.write_str(&format!("\"{s}\"")),
        }
    }
}
