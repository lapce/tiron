mod command;
mod copy;
pub mod data;
mod file;
mod git;
mod package;

use std::{collections::HashMap, fmt::Display, ops::Range};

use crossbeam_channel::Sender;
use itertools::Itertools;
use tiron_common::{
    action::{ActionId, ActionMessage},
    error::{Error, Origin},
    value::SpannedValue,
};

pub trait Action {
    /// name of the action
    fn name(&self) -> String;

    fn doc(&self) -> ActionDoc;

    fn input(&self, params: ActionParams) -> Result<Vec<u8>, Error>;

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
    fn parse_value(&self, value: &SpannedValue) -> Option<ActionParamBaseValue> {
        match self {
            ActionParamBaseType::String => {
                if let SpannedValue::String(s) = value {
                    return Some(ActionParamBaseValue::String(s.value().to_string()));
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
    fn parse_attr(&self, value: &SpannedValue) -> Option<ActionParamValue> {
        match self {
            ActionParamType::String => {
                if let SpannedValue::String(s) = value {
                    return Some(ActionParamValue::String(
                        s.value().to_string(),
                        value.span().to_owned(),
                    ));
                }
            }
            ActionParamType::Bool => {
                if let SpannedValue::Bool(v) = value {
                    return Some(ActionParamValue::Bool(*v.value()));
                }
            }
            ActionParamType::List(base) => {
                if let SpannedValue::Array(v) = value {
                    let mut items = Vec::new();
                    for v in v.value().iter() {
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
        origin: &Origin,
        attrs: &HashMap<String, SpannedValue>,
    ) -> Result<Option<ActionParamValue>, Error> {
        let param = attrs.get(&self.name);

        if let Some(param) = param {
            for type_ in &self.type_ {
                if let Some(value) = type_.parse_attr(param) {
                    return Ok(Some(value));
                }
            }
            return origin
                .error(
                    format!(
                        "{} type should be {}",
                        self.name,
                        self.type_.iter().map(|t| t.to_string()).join(" or ")
                    ),
                    param.span(),
                )
                .err();
        }

        if self.required {
            return Error::new(format!("can't find {} in params, it's required", self.name)).err();
        }

        Ok(None)
    }
}

pub struct ActionDoc {
    pub description: String,
    pub params: Vec<ActionParamDoc>,
}

impl ActionDoc {
    pub fn parse_attrs<'a>(
        &self,
        origin: &'a Origin,
        attrs: &HashMap<String, SpannedValue>,
    ) -> Result<ActionParams<'a>, Error> {
        let mut values = Vec::new();
        for param in &self.params {
            let value = param.parse_attrs(origin, attrs)?;
            values.push(value);
        }

        Ok(ActionParams {
            origin,
            span: None,
            values,
        })
    }
}

pub struct ActionParams<'a> {
    pub origin: &'a Origin,
    pub span: Option<Range<usize>>,
    pub values: Vec<Option<ActionParamValue>>,
}

impl<'a> ActionParams<'a> {
    pub fn expect_string(&self, i: usize) -> &str {
        self.values[i].as_ref().unwrap().expect_string()
    }

    pub fn expect_string_with_span(&self, i: usize) -> (&str, &Option<Range<usize>>) {
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
    String(String, Option<Range<usize>>),
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

    pub fn string_with_span(&self) -> Option<(&str, &Option<Range<usize>>)> {
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

    pub fn expect_string_with_span(&self) -> (&str, &Option<Range<usize>>) {
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
    fn match_value_new(&self, value: &SpannedValue) -> bool {
        match self {
            ActionParamBaseValue::String(base) => {
                if let SpannedValue::String(s) = value {
                    return base == s.value();
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
