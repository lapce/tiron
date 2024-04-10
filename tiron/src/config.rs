use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use rcl::{error::Error, loader::Loader, markup::MarkupMode, runtime::Value};
use tiron_tui::event::AppEvent;

use crate::{core::print_warn, node::Node};

pub enum HostOrGroup {
    Host(String),
    Group(String),
}

pub struct HostOrGroupConfig {
    host: HostOrGroup,
    vars: HashMap<String, Value>,
}

pub struct GroupConfig {
    hosts: Vec<HostOrGroupConfig>,
    vars: HashMap<String, Value>,
}

pub struct Config {
    pub tx: Sender<AppEvent>,
    groups: HashMap<String, GroupConfig>,
    pub project_folder: PathBuf,
}

impl Config {
    pub fn load(loader: &mut Loader, tx: &Sender<AppEvent>) -> Result<Config, Error> {
        let mut cwd = std::env::current_dir()
            .map_err(|e| Error::new(format!("can't get current directory {e}")))?;

        let mut path = cwd.join("tiron.rcl");
        while !path.exists() {
            cwd = cwd
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| Error::new("can't find tiron.rcl"))?;
            path = cwd.join("tiron.rcl");
        }

        let data = std::fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("can't reading config. Error: {e}")))?;

        let id = loader.load_string(data, Some(path.to_string_lossy().to_string()), 0);
        let value = loader.evaluate(
            &mut rcl::typecheck::prelude(),
            &mut rcl::runtime::prelude(),
            id,
            &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
        )?;

        let Value::Dict(mut dict, dict_span) = value else {
            return Error::new("root should be dict")
                .with_origin(*value.span())
                .err();
        };

        let mut config = Config {
            tx: tx.clone(),
            groups: HashMap::new(),
            project_folder: cwd,
        };

        if let Some(groups) = dict.remove(&Value::String("groups".into(), None)) {
            let Value::Dict(groups, groups_span) = groups else {
                return Error::new("hosts should be dict")
                    .with_origin(dict_span)
                    .err();
            };
            for (key, group) in groups.iter() {
                let Value::String(group_name, _) = key else {
                    return Error::new("group key should be a string")
                        .with_origin(groups_span)
                        .err();
                };
                let group = Self::parse_group(&groups, group_name, group)?;
                config.groups.insert(group_name.to_string(), group);
            }
        }

        for (key, _) in dict {
            let warn = Error::new("key here is unsed")
                .warning()
                .with_origin(*key.span());
            print_warn(warn, loader);
        }

        Ok(config)
    }

    fn parse_group(
        groups: &BTreeMap<Value, Value>,
        group_name: &str,
        value: &Value,
    ) -> Result<GroupConfig, Error> {
        let Value::Dict(group, group_span) = value else {
            return Error::new("group value should be a dict")
                .with_origin(*value.span())
                .err();
        };
        let mut group_config = GroupConfig {
            hosts: Vec::new(),
            vars: HashMap::new(),
        };
        let Some(group_hosts) = group.get(&Value::String("hosts".into(), None)) else {
            return Error::new("group should have hosts")
                .with_origin(*group_span)
                .err();
        };
        let Value::List(group_hosts) = group_hosts else {
            return Error::new("group value should be a list")
                .with_origin(*group_hosts.span())
                .err();
        };

        for host in group_hosts.iter() {
            let host_config = Self::parse_group_entry(groups, group_name, host)?;
            group_config.hosts.push(host_config);
        }

        if let Some(vars) = group.get(&Value::String("vars".into(), None)) {
            let Value::Dict(vars, _) = vars else {
                return Error::new("group entry vars should be a dict")
                    .with_origin(*vars.span())
                    .err();
            };
            for (key, var) in vars.iter() {
                let Value::String(key, _) = key else {
                    return Error::new("group entry vars key should be a string")
                        .with_origin(*key.span())
                        .err();
                };
                group_config.vars.insert(key.to_string(), var.clone());
            }
        }

        Ok(group_config)
    }

    fn parse_group_entry(
        groups: &BTreeMap<Value, Value>,
        group_name: &str,
        value: &Value,
    ) -> Result<HostOrGroupConfig, Error> {
        let Value::Dict(host, host_span) = value else {
            return Error::new("group entry should be a dict")
                .with_origin(*value.span())
                .err();
        };

        if host.contains_key(&Value::String("host".into(), None))
            && host.contains_key(&Value::String("group".into(), None))
        {
            return Error::new("group entry can't have host and group at the same time")
                .with_origin(*host_span)
                .err();
        }

        let host_or_group = if let Some(v) = host.get(&Value::String("host".into(), None)) {
            let Value::String(v, _) = v else {
                return Error::new("group entry host value should be a string")
                    .with_origin(*v.span())
                    .err();
            };
            HostOrGroup::Host(v.to_string())
        } else if let Some(v) = host.get(&Value::String("group".into(), None)) {
            let Value::String(v, v_span) = v else {
                return Error::new("group entry group value should be a string")
                    .with_origin(*v.span())
                    .err();
            };
            if v.as_ref() == group_name {
                return Error::new("group entry group can't point to itself")
                    .with_origin(*v_span)
                    .err();
            }
            if !groups.contains_key(&Value::String(v.clone(), None)) {
                return Error::new("group entry group doesn't exist")
                    .with_origin(*v_span)
                    .err();
            }

            HostOrGroup::Group(v.to_string())
        } else {
            return Error::new("group entry should have either host or group")
                .with_origin(*host_span)
                .err();
        };
        let mut host_config = HostOrGroupConfig {
            host: host_or_group,
            vars: HashMap::new(),
        };

        if let Some(vars) = host.get(&Value::String("vars".into(), None)) {
            let Value::Dict(vars, _) = vars else {
                return Error::new("group entry vars should be a dict")
                    .with_origin(*vars.span())
                    .err();
            };
            for (key, var) in vars.iter() {
                let Value::String(key, _) = key else {
                    return Error::new("group entry vars key should be a string")
                        .with_origin(*key.span())
                        .err();
                };
                host_config.vars.insert(key.to_string(), var.clone());
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
