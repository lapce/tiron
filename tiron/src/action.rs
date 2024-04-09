use std::{collections::HashMap, path::Path};

use anyhow::Result;
use rcl::runtime::Value;
use tiron_common::{
    action::{ActionData, ActionId},
    error::Error,
};
use tiron_node::action::data::all_actions;

use crate::job::Job;

pub fn parse_actions(
    cwd: &Path,
    value: &Value,
    vars: &HashMap<String, Value>,
) -> Result<Vec<ActionData>, Error> {
    let Value::List(action_values) = value else {
        return Error::new("actions should be a list", *value.span()).err();
    };

    let all_actions = all_actions();

    let mut actions = Vec::new();
    for action_value in action_values.iter() {
        let Value::Dict(dict, dict_span) = action_value else {
            return Error::new("action should be a dict", *value.span()).err();
        };
        let Some(action) = dict.get(&Value::String("action".into(), None)) else {
            return Error::new("missing action key in action", *dict_span).err();
        };
        let Value::String(action_name, action_name_span) = action else {
            return Error::new("action key should be string", *action.span()).err();
        };

        let name = if let Some(name) = dict.get(&Value::String("name".into(), None)) {
            let Value::String(name, _) = name else {
                return Error::new("name should be string", *name.span()).err();
            };
            Some(name.to_string())
        } else {
            None
        };

        if action_name.as_ref() == "job" {
            let Some(params) = dict.get(&Value::String("params".into(), None)) else {
                return Error::new("job needs params", *dict_span).err();
            };
            let Value::Dict(params, params_span) = params else {
                return Error::new("params should be a dict", *params.span()).err();
            };
            let Some(job_name) = params.get(&Value::String("name".into(), None)) else {
                return Error::new("missing job name in action", *params_span).err();
            };
            let Value::String(job_name, _) = job_name else {
                return Error::new("job name should be string", *job_name.span()).err();
            };
            let mut job_actions = Job::load(cwd, job_name, vars)?;
            actions.append(&mut job_actions);
        } else {
            let Some(action) = all_actions.get(action_name.as_ref()) else {
                return Error::new("action can't be found", *action_name_span).err();
            };
            let params = dict.get(&Value::String("params".into(), None));
            let input = action
                .input(cwd, params)
                .map_err(|e| Error::new(e.msg, e.span.or(*dict_span)))?;
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
