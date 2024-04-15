use std::{collections::HashMap, path::PathBuf};

use crossbeam_channel::Sender;
use tiron_tui::event::AppEvent;

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

pub struct Config {
    pub tx: Sender<AppEvent>,
    pub groups: HashMap<String, GroupConfig>,
    pub project_folder: PathBuf,
}

impl Config {}
