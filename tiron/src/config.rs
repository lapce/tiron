use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use rcl::{loader::Loader, markup::MarkupMode, runtime::Value};
use tiron_common::error::Error;
use tiron_tui::event::AppEvent;

use crate::node::Node;

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
    tx: Sender<AppEvent>,
    groups: HashMap<String, GroupConfig>,
}

impl Config {
    pub fn load(loader: &mut Loader, tx: &Sender<AppEvent>) -> Result<Config, Error> {
        let path = match std::env::current_dir() {
            Ok(path) => path.join("tiron.rcl"),
            Err(_) => PathBuf::from("tiron.rcl"),
        };
        let data = std::fs::read_to_string(path)
            .map_err(|e| Error::new(format!("can't reading config. Error: {e}",), None))?;

        let id = loader.load_string(data);
        let value = loader
            .evaluate(
                &mut rcl::typecheck::prelude(),
                &mut rcl::runtime::prelude(),
                id,
                &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
            )
            .map_err(|e| Error::new("", e.origin))?;

        let Value::Dict(value, dict_span) = value else {
            return Error::new("root should be dict", *value.span()).err();
        };

        let mut config = Config {
            tx: tx.clone(),
            groups: HashMap::new(),
        };

        if let Some(groups) = value.get(&Value::String("groups".into(), None)) {
            let Value::Dict(groups, groups_span) = groups else {
                return Error::new("hosts should be dict", dict_span).err();
            };
            for (key, group) in groups.iter() {
                let Value::String(group_name, _) = key else {
                    return Error::new("group key should be a string", *groups_span).err();
                };
                let group = Self::parse_group(groups, group_name, group)?;
                config.groups.insert(group_name.to_string(), group);
            }
        }

        Ok(config)
    }

    fn parse_group(
        groups: &BTreeMap<Value, Value>,
        group_name: &str,
        value: &Value,
    ) -> Result<GroupConfig, Error> {
        let Value::Dict(group, group_span) = value else {
            return Error::new("group value should be a dict", *value.span()).err();
        };
        let mut group_config = GroupConfig {
            hosts: Vec::new(),
            vars: HashMap::new(),
        };
        let Some(group_hosts) = group.get(&Value::String("hosts".into(), None)) else {
            return Error::new("group should have hosts", *group_span).err();
        };
        let Value::List(group_hosts) = group_hosts else {
            return Error::new("group value should be a list", *group_hosts.span()).err();
        };

        for host in group_hosts.iter() {
            let host_config = Self::parse_group_entry(groups, group_name, host)?;
            group_config.hosts.push(host_config);
        }

        if let Some(vars) = group.get(&Value::String("vars".into(), None)) {
            let Value::Dict(vars, vars_span) = vars else {
                return Error::new("group entry vars should be a dict", *vars.span()).err();
            };
            for (key, var) in vars.iter() {
                let Value::String(key, _) = key else {
                    return Error::new("group entry vars key should be a string", *vars_span).err();
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
            return Error::new("group entry should be a dict", *value.span()).err();
        };

        if host.contains_key(&Value::String("host".into(), None))
            && host.contains_key(&Value::String("group".into(), None))
        {
            return Error::new(
                "group entry can't have host and group at the same time",
                *host_span,
            )
            .err();
        }

        let host_or_group = if let Some(v) = host.get(&Value::String("host".into(), None)) {
            let Value::String(v, _) = v else {
                return Error::new("group entry host value should be a string", *v.span()).err();
            };
            HostOrGroup::Host(v.to_string())
        } else if let Some(v) = host.get(&Value::String("group".into(), None)) {
            let Value::String(v, v_span) = v else {
                return Error::new("group entry group value should be a string", *v.span()).err();
            };
            if v.as_ref() == group_name {
                return Error::new("group entry group can't point to itself", *v_span).err();
            }
            if !groups.contains_key(&Value::String(v.clone(), None)) {
                return Error::new("group entry group doesn't exist", *v_span).err();
            }

            HostOrGroup::Group(v.to_string())
        } else {
            return Error::new("group entry should have either host or group", *host_span).err();
        };
        let mut host_config = HostOrGroupConfig {
            host: host_or_group,
            vars: HashMap::new(),
        };

        if let Some(vars) = host.get(&Value::String("vars".into(), None)) {
            let Value::Dict(vars, _) = vars else {
                return Error::new("group entry vars should be a dict", *vars.span()).err();
            };
            for (key, var) in vars.iter() {
                let Value::String(key, _) = key else {
                    return Error::new("group entry vars key should be a string", *var.span())
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
