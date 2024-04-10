use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::Result;
use rcl::{
    error::Error,
    loader::Loader,
    markup::MarkupMode,
    pprint::Doc,
    runtime::Value,
    source::Span,
    types::{SourcedType, Type},
};
use tiron_tui::run::{ActionSection, HostSection, RunPanel};
use uuid::Uuid;

use crate::{action::parse_actions, config::Config, node::Node};

pub struct Run {
    pub id: Uuid,
    name: Option<String>,
    hosts: Vec<Node>,
}

impl Run {
    pub fn from_runbook(
        loader: &mut Loader,
        cwd: &Path,
        name: Option<String>,
        origin: Span,
        hosts: Vec<Node>,
        config: &Config,
    ) -> Result<Self, Error> {
        let doc = origin.doc();
        let start_line = {
            let doc = loader.get_doc(doc);
            origin.start_line(doc.data)
        };

        let hosts = if hosts.is_empty() {
            vec![Node {
                id: Uuid::new_v4(),
                host: "localhost".to_string(),
                vars: HashMap::new(),
                remote_user: None,
                actions: Vec::new(),
                tx: config.tx.clone(),
            }]
        } else {
            hosts
        };

        let mut run = Run {
            id: Uuid::new_v4(),
            name,
            hosts,
        };

        for host in run.hosts.iter_mut() {
            let doc = loader.get_doc(doc);
            let content = origin.resolve(doc.data);
            let id = loader.load_string(
                content.to_string(),
                Some(doc.name.to_string()),
                start_line.saturating_sub(1),
            );
            let mut type_env = rcl::typecheck::prelude();
            let mut env = rcl::runtime::prelude();
            for (name, value) in &host.vars {
                type_env.push(name.as_str().into(), value_to_type(value));
                env.push(name.as_str().into(), value.clone());
            }
            let value = loader.evaluate(
                &mut type_env,
                &mut env,
                id,
                &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
            )?;

            let Value::Dict(dict, dict_span) = value else {
                return Error::new("run should be a dict")
                    .with_origin(*value.span())
                    .err();
            };
            let Some(value) = dict.get(&Value::String("actions".into(), None)) else {
                return Error::new("run should have actions")
                    .with_origin(dict_span)
                    .err();
            };

            if let Some(remote_user) = dict.get(&Value::String("remote_user".into(), None)) {
                let Value::String(remote_user, _) = remote_user else {
                    return Error::new("remote_user should be a string")
                        .with_origin(*remote_user.span())
                        .err();
                };
                if host.remote_user.is_none() {
                    host.remote_user = Some(remote_user.to_string());
                }
            }

            let mut job_depth = 0;
            let actions = parse_actions(loader, cwd, value, &host.vars, &mut job_depth, config)
                .map_err(|mut e| {
                    e.message =
                        Doc::string(format!("parsing actions for host {} error: ", host.host))
                            + e.message;
                    e
                })?;
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
