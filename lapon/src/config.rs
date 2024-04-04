use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::{anyhow, Context, Result};
use rcl::{markup::MarkupMode, runtime::Value};

pub struct HostConfig {
    pub host: String,
    pub vars: HashMap<String, String>,
}

pub enum HostOrGroup {
    Host(String),
    Group(String),
}

pub struct HostOrGroupConfig {
    host: HostOrGroup,
    vars: HashMap<String, String>,
}

pub struct GroupConfig {
    hosts: Vec<HostOrGroupConfig>,
    vars: HashMap<String, String>,
}

pub struct Config {
    groups: HashMap<String, GroupConfig>,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = match std::env::current_dir() {
            Ok(path) => path.join("lapon.rcl"),
            Err(_) => PathBuf::from("lapon.rcl"),
        };
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("can't reading config {}", path.to_string_lossy()))?;

        let mut loader = rcl::loader::Loader::new();
        let id = loader.load_string(data);
        let value = loader
            .evaluate(
                &mut rcl::typecheck::prelude(),
                &mut rcl::runtime::prelude(),
                id,
                &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
            )
            .map_err(|e| {
                anyhow!(
                    "can't parse rcl file: {:?} {:?} {:?}",
                    e.message,
                    e.body,
                    e.origin
                )
            })?;

        let Value::Dict(value) = value else {
            return Err(anyhow!("invalid lapon.rcl: root should be dict"));
        };

        let mut config = Config {
            groups: HashMap::new(),
        };

        if let Some(groups) = value.get(&Value::String("groups".into())) {
            let Value::Dict(groups) = groups else {
                return Err(anyhow!("invalid lapon.rcl: hosts should be dict"));
            };
            for (key, group) in groups.iter() {
                let Value::String(group_name) = key else {
                    return Err(anyhow!("invalid lapon.rcl: group key should be a string"));
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
    ) -> Result<GroupConfig> {
        let Value::Dict(group) = value else {
            return Err(anyhow!("invalid lapon.rcl: group value should be a dict"));
        };
        let mut group_config = GroupConfig {
            hosts: Vec::new(),
            vars: HashMap::new(),
        };
        let Some(group_hosts) = group.get(&Value::String("hosts".into())) else {
            return Err(anyhow!("invalid lapon.rcl: group should have hosts"));
        };
        let Value::List(group_hosts) = group_hosts else {
            return Err(anyhow!("invalid lapon.rcl: group value should be a list"));
        };

        for host in group_hosts.iter() {
            let host_config = Self::parse_group_entry(groups, group_name, host)?;
            group_config.hosts.push(host_config);
        }

        if let Some(vars) = group.get(&Value::String("vars".into())) {
            let Value::Dict(vars) = vars else {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry {group_name} vars should be a dict"
                ));
            };
            for (key, var) in vars.iter() {
                let Value::String(key) = key else {
                    return Err(anyhow!(
                        "invalid lapon.rcl: group entry {group_name} vars key should be a string"
                    ));
                };
                let Value::String(var) = var else {
                    return Err(anyhow!(
                        "invalid lapon.rcl: group entry {group_name} vars {key} value should be a string"
                    ));
                };
                group_config.vars.insert(key.to_string(), var.to_string());
            }
        }

        Ok(group_config)
    }

    fn parse_group_entry(
        groups: &BTreeMap<Value, Value>,
        group_name: &str,
        value: &Value,
    ) -> Result<HostOrGroupConfig> {
        let Value::Dict(host) = value else {
            return Err(anyhow!("invalid lapon.rcl: group entry should be a dict"));
        };

        if host.contains_key(&Value::String("host".into()))
            && host.contains_key(&Value::String("group".into()))
        {
            return Err(anyhow!(
                "invalid lapon.rcl: group entry can't have host and group at the same time"
            ));
        }

        let host_or_group = if let Some(v) = host.get(&Value::String("host".into())) {
            let Value::String(v) = v else {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry host value should be a string"
                ));
            };
            HostOrGroup::Host(v.to_string())
        } else if let Some(v) = host.get(&Value::String("group".into())) {
            let Value::String(v) = v else {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry group value should be a string"
                ));
            };
            if v.as_ref() == group_name {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry group can't point to itself"
                ));
            }
            if !groups.contains_key(&Value::String(v.clone())) {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry group {v} doesn't exist"
                ));
            }

            HostOrGroup::Group(v.to_string())
        } else {
            return Err(anyhow!(
                "invalid lapon.rcl: group entry should have either host or group"
            ));
        };
        let mut host_config = HostOrGroupConfig {
            host: host_or_group,
            vars: HashMap::new(),
        };

        if let Some(vars) = host.get(&Value::String("vars".into())) {
            let Value::Dict(vars) = vars else {
                return Err(anyhow!(
                    "invalid lapon.rcl: group entry {group_name} vars should be a dict"
                ));
            };
            for (key, var) in vars.iter() {
                let Value::String(key) = key else {
                    return Err(anyhow!(
                        "invalid lapon.rcl: group entry {group_name} vars key should be a string"
                    ));
                };
                let Value::String(var) = var else {
                    return Err(anyhow!(
                        "invalid lapon.rcl: group entry {group_name} vars {key} value should be a string"
                    ));
                };
                host_config.vars.insert(key.to_string(), var.to_string());
            }
        }

        Ok(host_config)
    }

    pub fn hosts_from_name(&self, name: &str) -> Result<Vec<HostConfig>> {
        if self.groups.contains_key(name) {
            return self.hosts_from_group(name);
        } else {
            for group in self.groups.values() {
                for host in &group.hosts {
                    if let HostOrGroup::Host(host_name) = &host.host {
                        if host_name == name {
                            return Ok(vec![HostConfig {
                                host: host_name.to_string(),
                                vars: host.vars.clone(),
                            }]);
                        }
                    }
                }
            }
        }
        Err(anyhow!("can't find host with name {name}"))
    }

    fn hosts_from_group(&self, group: &str) -> Result<Vec<HostConfig>> {
        let Some(group) = self.groups.get(group) else {
            return Err(anyhow!("hosts doesn't have group {group}"));
        };

        let mut hosts = Vec::new();
        for host_or_group in &group.hosts {
            let mut local_hosts = match &host_or_group.host {
                HostOrGroup::Host(name) => {
                    let host_config = HostConfig {
                        host: name.to_string(),
                        vars: host_or_group.vars.clone(),
                    };
                    vec![host_config]
                }
                HostOrGroup::Group(group) => {
                    let mut local_hosts = self.hosts_from_group(group)?;
                    for host in local_hosts.iter_mut() {
                        for (key, val) in &host_or_group.vars {
                            if !host.vars.contains_key(key) {
                                host.vars.insert(key.to_string(), val.to_string());
                            }
                        }
                    }
                    local_hosts
                }
            };
            for host in local_hosts.iter_mut() {
                for (key, val) in &group.vars {
                    if !host.vars.contains_key(key) {
                        host.vars.insert(key.to_string(), val.to_string());
                    }
                }
            }
            hosts.append(&mut local_hosts);
        }
        Ok(hosts)
    }
}
