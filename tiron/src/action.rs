use std::collections::HashMap;

use anyhow::Result;
use hcl::eval::Context;
use hcl_edit::{
    structure::{Block, BlockLabel, Structure},
    Span,
};
use tiron_common::{
    action::{ActionData, ActionId},
    error::Error,
    value::SpannedValue,
};
use tiron_node::action::data::all_actions;

use crate::core::Runbook;

pub fn parse_actions(
    runbook: &Runbook,
    block: &Block,
    vars: &HashMap<String, hcl::Value>,
) -> Result<Vec<ActionData>, Error> {
    let all_actions = all_actions();

    let mut ctx = Context::new();
    for (name, var) in vars {
        ctx.declare_var(name.to_string(), var.to_owned());
    }

    let mut actions = Vec::new();
    for s in block.body.iter() {
        if let Structure::Block(block) = s {
            if block.ident.as_str() == "action" {
                if block.labels.is_empty() {
                    return runbook
                        .origin
                        .error("No action name", &block.ident.span())
                        .err();
                }
                if block.labels.len() > 1 {
                    return runbook
                        .origin
                        .error("You can only have one action name", &block.labels[1].span())
                        .err();
                }
                let BlockLabel::String(action_name) = &block.labels[0] else {
                    return runbook
                        .origin
                        .error("action name should be a string", &block.labels[0].span())
                        .err();
                };

                let params = block.body.iter().find_map(|s| {
                    s.as_block()
                        .filter(|&block| block.ident.as_str() == "params")
                });

                let name = block.body.iter().find_map(|s| {
                    s.as_attribute()
                        .filter(|a| a.key.as_str() == "name")
                        .map(|a| &a.value)
                });
                let name = if let Some(name) = name {
                    let name =
                        SpannedValue::from_expression(&runbook.origin, &ctx, name.to_owned())?;
                    let SpannedValue::String(s) = name else {
                        return runbook
                            .origin
                            .error("name should be a string", name.span())
                            .err();
                    };
                    Some(s.value().to_string())
                } else {
                    None
                };

                let params = params.ok_or_else(|| {
                    runbook
                        .origin
                        .error("action doesn't have params", &block.ident.span())
                })?;

                let mut attrs = HashMap::new();
                for s in params.body.iter() {
                    if let Some(a) = s.as_attribute() {
                        let v = SpannedValue::from_expression(
                            &runbook.origin,
                            &ctx,
                            a.value.to_owned(),
                        )?;
                        attrs.insert(a.key.to_string(), v);
                    }
                }

                if action_name.as_str() == "job" {
                    let job_name = attrs.get("name").ok_or_else(|| {
                        runbook
                            .origin
                            .error("job doesn't have name in params", &params.ident.span())
                    })?;
                    let SpannedValue::String(job_name) = job_name else {
                        return runbook
                            .origin
                            .error("job name should be a string", job_name.span())
                            .err();
                    };
                    let job = runbook.jobs.get(job_name.value()).ok_or_else(|| {
                        runbook.origin.error("can't find job name", job_name.span())
                    })?;

                    let runbook = if let Some(imported) = &job.imported {
                        runbook.imports.get(imported).ok_or_else(|| {
                            runbook
                                .origin
                                .error("can't find imported job", job_name.span())
                        })?
                    } else {
                        runbook
                    };

                    actions.append(&mut parse_actions(runbook, &job.block, vars)?);
                } else {
                    let Some(action) = all_actions.get(action_name.as_str()) else {
                        return runbook
                            .origin
                            .error(
                                format!("action {} can't be found", action_name.as_str()),
                                &block.labels[0].span(),
                            )
                            .err();
                    };

                    let params =
                        action
                            .doc()
                            .parse_attrs(&runbook.origin, &attrs)
                            .map_err(|e| {
                                let mut e = e;
                                if e.location.is_none() {
                                    e = e.with_origin(&runbook.origin, &params.ident.span());
                                }
                                e
                            })?;
                    let input = action.input(params)?;
                    actions.push(ActionData {
                        id: ActionId::new(),
                        name: name.unwrap_or_else(|| action_name.to_string()),
                        action: action_name.to_string(),
                        input,
                    });
                }
            }
        }
    }
    Ok(actions)
}

// pub fn parse_actions(
//     loader: &mut Loader,
//     cwd: &Path,
//     value: &Value,
//     vars: &HashMap<String, Value>,
//     job_depth: &mut i32,
//     config: &Config,
// ) -> Result<Vec<ActionData>, Error> {
//     let Value::List(action_values) = value else {
//         return Error::new("actions should be a list")
//             .with_origin(*value.span())
//             .err();
//     };

//     let all_actions = all_actions();

//     let mut actions = Vec::new();
//     for action_value in action_values.iter() {
//         let Value::Dict(dict, dict_span) = action_value else {
//             return Error::new("action should be a dict")
//                 .with_origin(*value.span())
//                 .err();
//         };
//         let Some(action) = dict.get(&Value::String("action".into(), None)) else {
//             return Error::new("missing action key in action")
//                 .with_origin(*dict_span)
//                 .err();
//         };
//         let Value::String(action_name, action_name_span) = action else {
//             return Error::new("action key should be string")
//                 .with_origin(*action.span())
//                 .err();
//         };

//         let name = if let Some(name) = dict.get(&Value::String("name".into(), None)) {
//             let Value::String(name, _) = name else {
//                 return Error::new("name should be string")
//                     .with_origin(*name.span())
//                     .err();
//             };
//             Some(name.to_string())
//         } else {
//             None
//         };

//         if action_name.as_ref() == "job" {
//             let Some(params) = dict.get(&Value::String("params".into(), None)) else {
//                 return Error::new("job needs params").with_origin(*dict_span).err();
//             };
//             let Value::Dict(params, params_span) = params else {
//                 return Error::new("params should be a dict")
//                     .with_origin(*params.span())
//                     .err();
//             };
//             let Some(job_name) = params.get(&Value::String("name".into(), None)) else {
//                 return Error::new("missing job name in action")
//                     .with_origin(*params_span)
//                     .err();
//             };
//             let Value::String(job_name, job_name_span) = job_name else {
//                 return Error::new("job name should be string")
//                     .with_origin(*job_name.span())
//                     .err();
//             };
//             *job_depth += 1;
//             if *job_depth > 500 {
//                 return Error::new("job name might have a endless loop here")
//                     .with_origin(*job_name_span)
//                     .err();
//             }
//             let mut job_actions = Job::load(
//                 loader,
//                 *job_name_span,
//                 cwd,
//                 job_name,
//                 vars,
//                 job_depth,
//                 config,
//             )?;
//             *job_depth -= 1;

//             actions.append(&mut job_actions);
//         } else {
//             let Some(action) = all_actions.get(action_name.as_ref()) else {
//                 return Error::new("action can't be found")
//                     .with_origin(*action_name_span)
//                     .err();
//             };
//             let params = dict.get(&Value::String("params".into(), None));
//             let params = action.doc().parse_params(params)?;
//             let input = action.input(cwd, params).map_err(|e| {
//                 let mut e = e;
//                 if e.origin.is_none() {
//                     e.origin = *dict_span
//                 }
//                 e
//             })?;
//             actions.push(ActionData {
//                 id: ActionId::new(),
//                 name: name.unwrap_or_else(|| action_name.to_string()),
//                 action: action_name.to_string(),
//                 input,
//             });
//         }
//     }
//     Ok(actions)
// }
