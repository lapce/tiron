use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use lapon_node::{
    action::{all_actions, ActionData},
    node::NodeMessage,
};
use rcl::runtime::Value;

use crate::{
    config::{Config, HostConfig},
    remote::{start_remote, SshHost, SshRemote},
};

pub struct Run {
    hosts: Vec<HostConfig>,
    remote_user: Option<String>,
    actions: Vec<ActionData>,
}

impl Run {
    pub fn from_value(cwd: &Path, config: &Config, value: &Value) -> Result<Self> {
        let Value::Dict(value) = value else {
            return Err(anyhow!("run should be a dict"));
        };

        let mut run = Run {
            hosts: Vec::new(),
            remote_user: None,
            actions: Vec::new(),
        };

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
            run.hosts.push(HostConfig {
                host: "localhost".to_string(),
                vars: HashMap::new(),
            });
        }

        if let Some(value) = value.get(&Value::String("remote_user".into())) {
            let Value::String(remote_user) = value else {
                return Err(anyhow!("remote_user should be string"));
            };
            run.remote_user = Some(remote_user.to_string());
        }

        if let Some(value) = value.get(&Value::String("actions".into())) {
            let actions = ActionData::parse_value(cwd, value)?;
            run.actions = actions;
        }

        Ok(run)
    }

    pub fn execute(&self) -> Result<()> {
        let mut senders = Vec::new();

        for host in &self.hosts {
            senders.push(start_remote(SshRemote {
                ssh: SshHost {
                    host: host.host.clone(),
                    port: None,
                    user: self.remote_user.clone(),
                },
            })?);
        }

        let all_actions = all_actions();
        for action_data in &self.actions {
            let Some(_) = all_actions.get(&action_data.name) else {
                return Err(anyhow!("action {} can't be found", action_data.name));
            };
            for (tx, _) in &senders {
                tx.send(NodeMessage::Action(action_data.clone()))?;
            }
        }

        for (tx, _) in &senders {
            tx.send(NodeMessage::Shutdown)?;
        }

        for (_, rx) in &senders {
            let _ = rx.recv();
        }

        Ok(())
    }
}
