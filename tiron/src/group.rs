use std::{collections::HashMap, path::PathBuf};

#[derive(Clone)]
pub enum HostOrGroup {
    Host(String),
    Group(String),
}

#[derive(Clone)]
pub struct HostOrGroupConfig {
    pub host: HostOrGroup,
    pub vars: HashMap<String, hcl::Value>,
}

#[derive(Clone)]
pub struct GroupConfig {
    pub hosts: Vec<HostOrGroupConfig>,
    pub vars: HashMap<String, hcl::Value>,
    pub imported: Option<PathBuf>,
}
