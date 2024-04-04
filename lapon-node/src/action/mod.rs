use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use rcl::runtime::Value;
use serde::{Deserialize, Serialize};

use crate::job::Job;

use self::copy::CopyAction;

mod copy;

pub trait Action {
    fn input(&self, cwd: &Path, value: &Value) -> anyhow::Result<Vec<u8>>;
    fn execute(&self, input: &[u8]) -> anyhow::Result<String>;
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ActionData {
    pub name: String,
    pub input: Vec<u8>,
}

impl ActionData {
    pub fn parse_value(cwd: &Path, value: &Value) -> Result<Vec<ActionData>> {
        let Value::List(action_values) = value else {
            return Err(anyhow!("actions should be a list"));
        };

        let all_actions = all_actions();

        let mut actions = Vec::new();
        for action_value in action_values.iter() {
            let Value::Dict(dict) = action_value else {
                return Err(anyhow!("action should be a dict"));
            };
            let Some(action) = dict.get(&Value::String("action".into())) else {
                return Err(anyhow!("missing action key in action"));
            };
            let Value::String(action_name) = action else {
                return Err(anyhow!("action key should be string"));
            };

            if action_name.as_ref() == "job" {
                let Some(job_name) = dict.get(&Value::String("job".into())) else {
                    return Err(anyhow!("missing job name in action"));
                };
                let Value::String(job_name) = job_name else {
                    return Err(anyhow!("job name should be string"));
                };
                let mut job_actions = Job::load(cwd, job_name)?;
                actions.append(&mut job_actions);
            } else {
                let Some(action) = all_actions.get(action_name.as_ref()) else {
                    return Err(anyhow!("action {action_name} can't be found"));
                };
                let input = action.input(cwd, action_value)?;
                actions.push(ActionData {
                    name: action_name.to_string(),
                    input,
                });
            }
        }
        Ok(actions)
    }
}

pub fn all_actions() -> HashMap<String, Box<dyn Action>> {
    [(
        "copy".to_string(),
        Box::new(CopyAction {}) as Box<dyn Action>,
    )]
    .into_iter()
    .collect()
}
