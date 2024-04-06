use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use rcl::runtime::Value;
use tiron_tui::{
    event::AppEvent,
    run::{ActionSection, HostSection, RunPanel},
};
use uuid::Uuid;

use crate::{action::parse_actions, config::Config, node::Node};

pub struct Run {
    pub id: Uuid,
    name: Option<String>,
    tx: Sender<AppEvent>,
    hosts: Vec<Node>,
    remote_user: Option<String>,
}

impl Run {
    pub fn from_value(
        cwd: &Path,
        config: &Config,
        value: &Value,
        tx: &Sender<AppEvent>,
    ) -> Result<Self> {
        let Value::Dict(value) = value else {
            return Err(anyhow!("run should be a dict"));
        };

        let mut run = Run {
            id: Uuid::new_v4(),
            name: None,
            tx: tx.clone(),
            hosts: Vec::new(),
            remote_user: None,
        };

        if let Some(name) = value.get(&Value::String("name".into())) {
            let Value::String(name) = name else {
                return Err(anyhow!("name should be string"));
            };
            run.name = Some(name.to_string());
        }

        if let Some(value) = value.get(&Value::String("hosts".into())) {
            if let Value::String(v) = value {
                run.hosts.append(&mut config.hosts_from_name(v)?);
            } else if let Value::List(v) = value {
                for host in v.iter() {
                    let Value::String(v) = host else {
                        return Err(anyhow!("hosts should be list of strings"));
                    };
                    run.hosts.append(&mut config.hosts_from_name(v)?);
                }
            } else {
                return Err(anyhow!("hosts should be either string or list"));
            };
        } else {
            run.hosts.push(Node {
                id: Uuid::new_v4(),
                host: "localhost".to_string(),
                vars: HashMap::new(),
                remote_user: None,
                actions: Vec::new(),
                tx: run.tx.clone(),
            });
        }

        if let Some(value) = value.get(&Value::String("remote_user".into())) {
            let Value::String(remote_user) = value else {
                return Err(anyhow!("remote_user should be string"));
            };
            run.remote_user = Some(remote_user.to_string());
        }

        let Some(value) = value.get(&Value::String("actions".into())) else {
            return Err(anyhow!("run should have actions"));
        };

        for host in run.hosts.iter_mut() {
            let actions = parse_actions(cwd, value, &host.vars)?;
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
