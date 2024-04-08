use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use rcl::runtime::Value;
use tiron_common::action::{ActionData, ActionId};
use tiron_node::action::data::all_actions;

use crate::job::Job;

pub fn parse_actions(
    cwd: &Path,
    value: &Value,
    vars: &HashMap<String, Value>,
) -> Result<Vec<ActionData>> {
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

        let name = if let Some(name) = dict.get(&Value::String("name".into())) {
            let Value::String(name) = name else {
                return Err(anyhow!("name should be string"));
            };
            Some(name.to_string())
        } else {
            None
        };

        if action_name.as_ref() == "job" {
            let Some(params) = dict.get(&Value::String("params".into())) else {
                return Err(anyhow!("job needs params"));
            };
            let Value::Dict(params) = params else {
                return Err(anyhow!("params should be a dict"));
            };
            let Some(job_name) = params.get(&Value::String("name".into())) else {
                return Err(anyhow!("missing job name in action"));
            };
            let Value::String(job_name) = job_name else {
                return Err(anyhow!("job name should be string"));
            };
            let mut job_actions = Job::load(cwd, job_name, vars)?;
            actions.append(&mut job_actions);
        } else {
            let Some(action) = all_actions.get(action_name.as_ref()) else {
                return Err(anyhow!("action {action_name} can't be found"));
            };
            let params = dict.get(&Value::String("params".into()));
            let input = action.input(cwd, params)?;
            actions.push(ActionData {
                id: ActionId::new(),
                name: name.unwrap_or_else(|| action_name.to_string()),
                action: action_name.to_string(),
                input,
            });
        }
    }
    Ok(actions)
}
