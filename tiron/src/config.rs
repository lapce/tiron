use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use hcl::eval::{Context, Evaluate};
use hcl_edit::structure::{Block, BlockLabel, Structure};
use rcl::{error::Error, loader::Loader, runtime::Value};
use tiron_tui::event::AppEvent;

use crate::node::Node;

#[derive(Clone)]
pub enum HostOrGroup {
    Host(String),
    Group(String),
}

#[derive(Clone)]
pub struct HostOrGroupConfig {
    pub host: HostOrGroup,
    pub vars: HashMap<String, Value>,
    pub new_vars: HashMap<String, hcl::Value>,
}

#[derive(Clone)]
pub struct GroupConfig {
    pub hosts: Vec<HostOrGroupConfig>,
    pub vars: HashMap<String, Value>,
    pub new_vars: HashMap<String, hcl::Value>,
}

pub struct Config {
    pub tx: Sender<AppEvent>,
    pub groups: HashMap<String, GroupConfig>,
    pub project_folder: PathBuf,
}

impl Config {
    pub fn load(loader: &mut Loader, tx: &Sender<AppEvent>) -> Result<Config, Error> {
        let mut cwd = std::env::current_dir()
            .map_err(|e| Error::new(format!("can't get current directory {e}")))?;

        let mut path = cwd.join("tiron.tr");
        while !path.exists() {
            cwd = cwd
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| Error::new("can't find tiron.tr"))?;
            path = cwd.join("tiron.tr");
        }

        let data = std::fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("can't reading config. Error: {e}")))?;

        let mut config = Config {
            tx: tx.clone(),
            groups: HashMap::new(),
            project_folder: cwd,
        };
        let group_configs = Self::parse_groups(&data)?;
        for (name, group) in group_configs {
            config.groups.insert(name, group);
        }

        Ok(config)
    }

    pub fn parse_groups(input: &str) -> Result<HashMap<String, GroupConfig>, Error> {
        let body = hcl_edit::parser::parse_body(input)
            .map_err(|e| Error::new(e.to_string().replace('\n', " ")))?;

        let mut groups = HashMap::new();
        for structure in body.iter() {
            match structure {
                Structure::Attribute(_) => {}
                Structure::Block(block) => {
                    if block.ident.as_str() == "group" {
                        if block.labels.is_empty() {
                            return Error::new("group name doesn't exit").err();
                        }
                        if block.labels.len() > 1 {
                            return Error::new("group should only have one name").err();
                        }
                        match &block.labels[0] {
                            BlockLabel::Ident(_) => {
                                return Error::new("group name should be a string").err();
                            }
                            BlockLabel::String(name) => {
                                let name = name.as_str();
                                if groups.contains_key(name) {
                                    return Error::new(
                                        "You can't define the same group name multipe numbers",
                                    )
                                    .err();
                                }
                                groups.insert(name, block);
                            }
                        }
                    }
                }
            }
        }

        let mut group_configs = HashMap::new();
        for (name, group) in &groups {
            let group = Self::parse_group(&groups, name, group)?;
            group_configs.insert(name.to_string(), group);
        }

        Ok(group_configs)
    }

    fn parse_group(
        groups: &HashMap<&str, &Block>,
        group_name: &str,
        block: &Block,
    ) -> Result<GroupConfig, Error> {
        let mut group_config = GroupConfig {
            hosts: Vec::new(),
            vars: HashMap::new(),
            new_vars: HashMap::new(),
        };

        let ctx = Context::new();
        for structure in block.body.iter() {
            match structure {
                Structure::Attribute(a) => {
                    let expr: hcl::Expression = a.value.to_owned().into();
                    let v: hcl::Value = expr
                        .evaluate(&ctx)
                        .map_err(|e| Error::new(e.to_string().replace('\n', " ")))?;
                    group_config.new_vars.insert(a.key.to_string(), v);
                }
                Structure::Block(block) => {
                    let host_or_group = Self::parse_group_entry(groups, group_name, block)?;
                    group_config.hosts.push(host_or_group);
                }
            }
        }

        Ok(group_config)
    }

    fn parse_group_entry(
        groups: &HashMap<&str, &Block>,
        group_name: &str,
        block: &Block,
    ) -> Result<HostOrGroupConfig, Error> {
        let ident = block.ident.as_str();
        let host_or_group = match ident {
            "host" => {
                if block.labels.is_empty() {
                    return Error::new("host name doesn't exit").err();
                }
                if block.labels.len() > 1 {
                    return Error::new("host should only have one name").err();
                }

                let BlockLabel::String(name) = &block.labels[0] else {
                    return Error::new("host name should be a string").err();
                };

                HostOrGroup::Host(name.to_string())
            }
            "group" => {
                if block.labels.is_empty() {
                    return Error::new("group name doesn't exit").err();
                }
                if block.labels.len() > 1 {
                    return Error::new("group should only have one name").err();
                }

                let BlockLabel::String(name) = &block.labels[0] else {
                    return Error::new("group name should be a string").err();
                };

                if name.as_str() == group_name {
                    return Error::new("group can't point to itself").err();
                }

                if !groups.contains_key(name.as_str()) {
                    return Error::new(format!("group {} doesn't exist", name.as_str())).err();
                }

                HostOrGroup::Group(name.to_string())
            }
            _ => return Error::new("you can only have host or group").err(),
        };

        let mut host_config = HostOrGroupConfig {
            host: host_or_group,
            vars: HashMap::new(),
            new_vars: HashMap::new(),
        };

        let ctx = Context::new();
        for structure in block.body.iter() {
            if let Structure::Attribute(a) = structure {
                let expr: hcl::Expression = a.value.to_owned().into();
                let v: hcl::Value = expr
                    .evaluate(&ctx)
                    .map_err(|e| Error::new(e.to_string().replace('\n', " ")))?;
                host_config.new_vars.insert(a.key.to_string(), v);
            }
        }

        Ok(host_config)
    }

    pub fn hosts_from_name(&self, name: &str) -> Result<Vec<Node>> {
        if self.groups.contains_key(name) {
            return self.hosts_from_group(name);
        } else {
            for group in self.groups.values() {
                for host in &group.hosts {
                    if let HostOrGroup::Host(host_name) = &host.host {
                        if host_name == name {
                            return Ok(vec![Node::new(
                                host_name.to_string(),
                                host.vars.clone(),
                                host.new_vars.clone(),
                                &self.tx,
                            )]);
                        }
                    }
                }
            }
        }
        Err(anyhow!("can't find host with name {name}"))
    }

    fn hosts_from_group(&self, group: &str) -> Result<Vec<Node>> {
        let Some(group) = self.groups.get(group) else {
            return Err(anyhow!("hosts doesn't have group {group}"));
        };

        let mut hosts = Vec::new();
        for host_or_group in &group.hosts {
            let mut local_hosts = match &host_or_group.host {
                HostOrGroup::Host(name) => {
                    vec![Node::new(
                        name.to_string(),
                        host_or_group.vars.clone(),
                        host_or_group.new_vars.clone(),
                        &self.tx,
                    )]
                }
                HostOrGroup::Group(group) => {
                    let mut local_hosts = self.hosts_from_group(group)?;
                    for host in local_hosts.iter_mut() {
                        for (key, val) in &host_or_group.vars {
                            if !host.vars.contains_key(key) {
                                if key == "remote_user" && host.remote_user.is_none() {
                                    host.remote_user = if let Value::String(s, _) = val {
                                        Some(s.to_string())
                                    } else {
                                        None
                                    };
                                }
                                host.vars.insert(key.to_string(), val.clone());
                            }
                        }
                    }
                    local_hosts
                }
            };
            for host in local_hosts.iter_mut() {
                for (key, val) in &group.vars {
                    if !host.vars.contains_key(key) {
                        if key == "remote_user" && host.remote_user.is_none() {
                            host.remote_user = if let Value::String(s, _) = val {
                                Some(s.to_string())
                            } else {
                                None
                            };
                        }
                        host.vars.insert(key.to_string(), val.clone());
                    }
                }
            }
            hosts.append(&mut local_hosts);
        }
        Ok(hosts)
    }
}
