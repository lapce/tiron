use crossbeam_channel::Sender;
use tiron_common::{
    action::{ActionId, ActionMessage},
    error::Error,
};

mod command;
mod copy;
pub mod data;
mod package;

pub trait Action {
    /// name of the action
    fn name(&self) -> String;

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

    fn doc(&self) -> ActionDoc;
}

pub enum ActionParamBaseType {
    String,
}

pub enum ActionParamType {
    String,
    Boolean,
    List(ActionParamBaseType),
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
