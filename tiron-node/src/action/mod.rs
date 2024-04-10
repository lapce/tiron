mod command;
mod copy;
pub mod data;
mod file;
mod git;
mod package;

use std::fmt::Display;

use crossbeam_channel::Sender;
use rcl::error::Error;
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
}

impl Display for ActionParamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionParamType::String => f.write_str("String"),
            ActionParamType::Boolean => f.write_str("Boolean"),
            ActionParamType::List(t) => f.write_str(&format!("List of {t}")),
        }
    }
}

pub struct ActionParamDoc {
    pub name: String,
    pub required: bool,
    pub type_: Vec<ActionParamType>,
    pub description: String,
}

pub struct ActionDoc {
    pub description: String,
    pub params: Vec<ActionParamDoc>,
}
