use anyhow::Result;
use hcl_edit::structure::Block;
use tiron_common::error::Error;
use tiron_tui::run::{ActionSection, HostSection, RunPanel};
use uuid::Uuid;

use crate::{action::parse_actions, core::Runbook, node::Node};

pub struct Run {
    pub id: Uuid,
    name: Option<String>,
    hosts: Vec<Node>,
}

impl Run {
    pub fn from_block(
        runbook: &Runbook,
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
            let actions = parse_actions(runbook, block, &host.vars).map_err(|e| {
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
