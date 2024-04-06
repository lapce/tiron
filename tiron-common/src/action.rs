use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Deserialize, Serialize)]
pub struct ActionId(Uuid);

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Deserialize, Serialize)]
pub enum ActionMessage {
    ActionStarted { id: ActionId },
    ActionStdout { id: ActionId, content: String },
    ActionStderr { id: ActionId, content: String },
    ActionResult { id: ActionId, success: bool },
    NodeShutdown { success: bool },
}

/// ActionData is the data that's being sent from core to node
/// with the input serialized
#[derive(Clone, Deserialize, Serialize)]
pub struct ActionData {
    pub id: ActionId,
    pub name: String,
    pub action: String,
    pub input: Vec<u8>,
}

/// ActionOutput is the output that's returned from the node
/// from executing the action
#[derive(Clone, Deserialize, Serialize, Default)]
pub struct ActionOutput {
    pub started: bool,
    pub lines: Vec<ActionOutputLine>,
    // whether this action was succesfully or not
    // the action isn't completed if this is None
    pub success: Option<bool>,
}

/// ActionOutputLine is one line for the ActionOutput
#[derive(Clone, Deserialize, Serialize)]
pub struct ActionOutputLine {
    pub content: String,
    pub level: ActionOutputLevel,
}

/// ActionOutputLevel indicates the severity of line in the output
#[derive(Clone, Deserialize, Serialize)]
pub enum ActionOutputLevel {
    Info,
    Warn,
    Error,
}
