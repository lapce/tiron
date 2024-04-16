use anyhow::Result;
use hcl::eval::Context;
use hcl_edit::{
    structure::{Block, Structure},
    Span,
};
use tiron_common::{error::Error, value::SpannedValue};
use tiron_tui::run::{ActionSection, HostSection, RunPanel};
use uuid::Uuid;

use crate::{node::Node, runbook::Runbook};

pub struct Run {
    pub id: Uuid,
    name: Option<String>,
    hosts: Vec<Node>,
}

impl Run {
    pub fn from_block(runbook: &Runbook, block: &Block, hosts: Vec<Node>) -> Result<Self, Error> {
        let name = block.body.iter().find_map(|s| {
            s.as_attribute()
                .filter(|a| a.key.as_str() == "name")
                .map(|a| &a.value)
        });
        let name = if let Some(name) = name {
            let hcl_edit::expr::Expression::String(s) = name else {
                return runbook
                    .origin
                    .error("name should be a string", &name.span())
                    .err();
            };
            Some(s.value().to_string())
        } else {
            None
        };

        let mut run = Run {
            id: Uuid::new_v4(),
            name,
            hosts,
        };

        for host in run.hosts.iter_mut() {
            let mut ctx = Context::new();
            for (name, var) in &host.vars {
                ctx.declare_var(name.to_string(), var.to_owned());
            }

            for s in block.body.iter() {
                if let Structure::Attribute(a) = s {
                    let v =
                        SpannedValue::from_expression(&runbook.origin, &ctx, a.value.to_owned())?;
                    match a.key.as_str() {
                        "remote_user" => {
                            if !host.vars.contains_key("remote_user") {
                                let SpannedValue::String(s) = v else {
                                    return runbook
                                        .origin
                                        .error("remote_user should be a string", v.span())
                                        .err();
                                };
                                host.remote_user = Some(s.value().to_string());
                            }
                        }
                        "become" => {
                            if !host.vars.contains_key("become") {
                                let SpannedValue::Bool(b) = v else {
                                    return runbook
                                        .origin
                                        .error("become should be a bool", v.span())
                                        .err();
                                };
                                host.become_ = *b.value();
                            }
                        }
                        _ => {}
                    }
                }
            }

            let actions = runbook.parse_actions(&ctx, block).map_err(|e| {
                let mut e = e;
                e.message = format!(
                    "error when parsing actions for host {}: {}",
                    host.host, e.message
                );
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
