use serde::{Deserialize, Serialize};

use crate::action::ActionData;

#[derive(Deserialize, Serialize)]
pub enum NodeMessage {
    Action(ActionData),
    Shutdown,
}
