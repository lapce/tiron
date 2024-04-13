use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::Result;
use hcl_edit::structure::Block;
use rcl::{
    loader::Loader,
    markup::MarkupMode,
    pprint::Doc,
    runtime::Value,
    source::Span,
    types::{SourcedType, Type},
};
use tiron_common::error::{Error, Origin};
use tiron_tui::run::{ActionSection, HostSection, RunPanel};
use uuid::Uuid;

use crate::{action::parse_actions_new, config::Config, node::Node};

pub struct Run {
    pub id: Uuid,
    name: Option<String>,
    hosts: Vec<Node>,
}

impl Run {
    pub fn from_block(
        origin: &Origin,
        name: Option<String>,
        block: &Block,
        hosts: Vec<Node>,
    ) -> Result<Self, Error> {
        let mut run = Run {
            id: Uuid::new_v4(),
            name,
            hosts,
        };

        for host in run.hosts.iter_mut() {
            let mut job_depth = 0;
            let actions = parse_actions_new(origin, block, &host.new_vars, &mut job_depth)?;
            host.actions = actions;
        }

        Ok(run)
    }

    pub fn execute(&self) -> Result<bool> {
        let mut receivers = Vec::new();

        for host in &self.hosts {
            let (exit_tx, exit_rx) = crossbeam_channel::bounded::<bool>(1);
            let host = host.clone();
            let run_id = self.id;
            std::thread::spawn(move || {
                let _ = host.execute(run_id, exit_tx);
            });

            receivers.push(exit_rx)
        }

        let mut errors = 0;
        for rx in &receivers {
            let result = rx.recv();
            if result != Ok(true) {
                errors += 1;
            }
        }

        Ok(errors == 0)
    }

    pub fn to_panel(&self) -> RunPanel {
        let hosts = self
            .hosts
            .iter()
            .map(|host| {
                HostSection::new(
                    host.id,
                    host.host.clone(),
                    host.actions
                        .iter()
                        .map(|action| ActionSection::new(action.id, action.name.clone()))
                        .collect(),
                )
            })
            .collect();
        RunPanel::new(self.id, self.name.clone(), hosts)
    }
}

pub fn value_to_type(value: &Value) -> SourcedType {
    let type_ = match value {
        Value::Null => Type::Null,
        Value::Bool(_) => Type::Bool,
        Value::Int(_) => Type::Int,
        Value::String(_, _) => Type::String,
        Value::List(list) => Type::List(Arc::new(if let Some(v) = list.first() {
            value_to_type(v)
        } else {
            SourcedType::any()
        })),
        Value::Set(v) => Type::Set(Arc::new(if let Some(v) = v.first() {
            value_to_type(v)
        } else {
            SourcedType::any()
        })),
        Value::Dict(v, _) => {
            Type::Dict(Arc::new(if let Some((key, value)) = v.first_key_value() {
                rcl::types::Dict {
                    key: value_to_type(key),
                    value: value_to_type(value),
                }
            } else {
                rcl::types::Dict {
                    key: SourcedType::any(),
                    value: SourcedType::any(),
                }
            }))
        }
        Value::Function(f) => Type::Function(f.type_.clone()),
        Value::BuiltinFunction(f) => Type::Function(Arc::new((f.type_)())),
        Value::BuiltinMethod(_) => Type::Any,
    };

    SourcedType {
        type_,
        source: rcl::type_source::Source::None,
    }
}
