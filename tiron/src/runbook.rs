use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use hcl::eval::{Context, Evaluate};
use hcl_edit::{
    structure::{Block, BlockLabel, Structure},
    Span,
};
use tiron_common::{
    action::{ActionData, ActionId},
    error::{Error, Origin},
    value::SpannedValue,
};
use tiron_node::action::data::all_actions;
use tiron_tui::event::AppEvent;
use uuid::Uuid;

use crate::{
    group::{GroupConfig, HostOrGroup, HostOrGroupConfig},
    job::Job,
    node::Node,
    run::Run,
};

pub struct Runbook {
    groups: HashMap<String, GroupConfig>,
    pub jobs: HashMap<String, Job>,
    // the imported runbooks
    pub imports: HashMap<PathBuf, Runbook>,
    pub runs: Vec<Run>,
    // the origin data of the runbook
    pub origin: Origin,
    tx: Sender<AppEvent>,
    // the imported level of the runbook, this is to detect circular imports
    level: usize,
}

impl Runbook {
    pub fn new(path: PathBuf, tx: Sender<AppEvent>, level: usize) -> Result<Self, Error> {
        let cwd = path.parent().ok_or_else(|| {
            Error::new(format!("can't find parent for {}", path.to_string_lossy()))
        })?;

        let data = std::fs::read_to_string(&path).map_err(|e| {
            Error::new(format!(
                "can't read runbook {} error: {e}",
                path.to_string_lossy()
            ))
        })?;

        let origin = Origin {
            cwd: cwd.to_path_buf(),
            path,
            data,
        };
        let runbook = Self {
            origin,
            groups: HashMap::new(),
            jobs: HashMap::new(),
            imports: HashMap::new(),
            runs: Vec::new(),
            tx,
            level,
        };

        Ok(runbook)
    }

    pub fn parse(&mut self, parse_run: bool) -> Result<(), Error> {
        let body = hcl_edit::parser::parse_body(&self.origin.data)
            .map_err(|e| Error::from_hcl(e, self.origin.path.clone()))?;

        for structure in body.iter() {
            if let Structure::Block(block) = structure {
                match block.ident.as_str() {
                    "use" => {
                        self.parse_use(block)?;
                    }
                    "group" => {
                        self.parse_group(block)?;
                    }
                    "job" => {
                        self.parse_job(block)?;
                    }
                    "run" => {
                        if parse_run {
                            // for imported runbook, we don't need to parse runs
                            self.parse_run(block)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn parse_run(&mut self, block: &Block) -> Result<(), Error> {
        let mut hosts: Vec<Node> = Vec::new();
        if block.labels.is_empty() {
            return self
                .origin
                .error("You need put group name after run", &block.ident.span())
                .err();
        }
        if block.labels.len() > 1 {
            return self
                .origin
                .error(
                    "You can only have one group name to run",
                    &block.labels[1].span(),
                )
                .err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return self
                .origin
                .error("group name should be a string", &block.labels[0].span())
                .err();
        };
        for node in self
            .hosts_from_name(name.as_str())
            .map_err(|e| self.origin.error(e.to_string(), &block.labels[0].span()))?
        {
            if !hosts.iter().any(|n| n.host == node.host) {
                hosts.push(node);
            }
        }

        let hosts = if hosts.is_empty() {
            vec![Node {
                id: Uuid::new_v4(),
                host: "localhost".to_string(),
                vars: HashMap::new(),
                remote_user: None,
                become_: false,
                actions: Vec::new(),
                tx: self.tx.clone(),
            }]
        } else {
            hosts
        };
        let run = Run::from_block(self, block, hosts)?;
        self.runs.push(run);
        Ok(())
    }

    fn parse_group(&mut self, block: &Block) -> Result<(), Error> {
        if block.labels.is_empty() {
            return self
                .origin
                .error("group name doesn't exit", &block.ident.span())
                .err();
        }
        if block.labels.len() > 1 {
            return self
                .origin
                .error("group should only have one name", &block.labels[1].span())
                .err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return self
                .origin
                .error("group name should be a string", &block.labels[0].span())
                .err();
        };

        if self.groups.contains_key(name.as_str()) {
            return self
                .origin
                .error("group name already exists", &block.labels[0].span())
                .err();
        }

        let mut group_config = GroupConfig {
            hosts: Vec::new(),
            vars: HashMap::new(),
            imported: None,
        };

        let ctx = Context::new();
        for structure in block.body.iter() {
            match structure {
                Structure::Attribute(a) => {
                    let expr: hcl::Expression = a.value.to_owned().into();
                    let v: hcl::Value = expr
                        .evaluate(&ctx)
                        .map_err(|e| Error::new(e.to_string().replace('\n', " ")))?;
                    group_config.vars.insert(a.key.to_string(), v);
                }
                Structure::Block(block) => {
                    let host_or_group = self.parse_group_entry(name, block)?;
                    group_config.hosts.push(host_or_group);
                }
            }
        }

        self.groups.insert(name.to_string(), group_config);

        Ok(())
    }

    fn parse_group_entry(
        &self,
        group_name: &str,
        block: &Block,
    ) -> Result<HostOrGroupConfig, Error> {
        let host_or_group = match block.ident.as_str() {
            "host" => {
                if block.labels.is_empty() {
                    return self
                        .origin
                        .error("host name doesn't exit", &block.ident.span())
                        .err();
                }
                if block.labels.len() > 1 {
                    return self
                        .origin
                        .error("host should only have one name", &block.labels[1].span())
                        .err();
                }

                let BlockLabel::String(name) = &block.labels[0] else {
                    return self
                        .origin
                        .error("host name should be a string", &block.labels[0].span())
                        .err();
                };

                HostOrGroup::Host(name.to_string())
            }
            "group" => {
                if block.labels.is_empty() {
                    return self
                        .origin
                        .error("group name doesn't exit", &block.ident.span())
                        .err();
                }
                if block.labels.len() > 1 {
                    return self
                        .origin
                        .error("group should only have one name", &block.labels[1].span())
                        .err();
                }

                let BlockLabel::String(name) = &block.labels[0] else {
                    return self
                        .origin
                        .error("group name should be a string", &block.labels[0].span())
                        .err();
                };

                if name.as_str() == group_name {
                    return self
                        .origin
                        .error("group can't point to itself", &block.labels[0].span())
                        .err();
                }

                if !self.groups.contains_key(name.as_str()) {
                    return self
                        .origin
                        .error(
                            format!("group {} doesn't exist", name.as_str()),
                            &block.labels[0].span(),
                        )
                        .err();
                }

                HostOrGroup::Group(name.to_string())
            }
            _ => {
                return self
                    .origin
                    .error("you can only have host or group", &block.ident.span())
                    .err()
            }
        };

        let mut host_config = HostOrGroupConfig {
            host: host_or_group,
            vars: HashMap::new(),
        };

        let ctx = Context::new();
        for structure in block.body.iter() {
            if let Structure::Attribute(a) = structure {
                let expr: hcl::Expression = a.value.to_owned().into();
                let v: hcl::Value = expr
                    .evaluate(&ctx)
                    .map_err(|e| Error::new(e.to_string().replace('\n', " ")))?;
                host_config.vars.insert(a.key.to_string(), v);
            }
        }

        Ok(host_config)
    }

    fn parse_use(&mut self, block: &Block) -> Result<(), Error> {
        if block.labels.is_empty() {
            return self
                .origin
                .error("use needs a path", &block.ident.span())
                .err();
        }
        if block.labels.len() > 1 {
            return self
                .origin
                .error(
                    "You can only have one path for use",
                    &block.labels[1].span(),
                )
                .err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return self
                .origin
                .error("path should be a string", &block.labels[0].span())
                .err();
        };

        let path = self.origin.cwd.join(name.as_str());

        let mut runbook = Runbook::new(path, self.tx.clone(), self.level + 1)?;
        runbook.parse(false).map_err(|e| {
            let mut e = e;
            if e.location.is_none() {
                e = e.with_origin(&self.origin, &block.labels[0].span());
            }
            e
        })?;

        let path = self
            .origin
            .cwd
            .join(name.as_str())
            .canonicalize()
            .map_err(|e| {
                Error::new(format!("can't canonicalize path: {e}"))
                    .with_origin(&self.origin, &block.labels[0].span())
            })?;
        if self.imports.contains_key(&path) {
            return self
                .origin
                .error("path already imported", &block.labels[0].span())
                .err();
        }

        for structure in block.body.iter() {
            if let Structure::Block(block) = structure {
                match block.ident.as_str() {
                    "job" => {
                        self.parse_use_job(&runbook, block)?;
                    }
                    "group" => {
                        self.parse_use_group(&runbook, block)?;
                    }
                    _ => {}
                }
            }
        }

        self.imports.insert(path, runbook);

        Ok(())
    }

    fn parse_use_job(&mut self, imported: &Runbook, block: &Block) -> Result<(), Error> {
        if block.labels.is_empty() {
            return self
                .origin
                .error("use job needs a job name", &block.ident.span())
                .err();
        }
        if block.labels.len() > 1 {
            return self
                .origin
                .error("You can only use one job name", &block.labels[1].span())
                .err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return self
                .origin
                .error("job name should be a string", &block.labels[0].span())
                .err();
        };

        let as_name = block.body.iter().find_map(|s| {
            s.as_attribute().and_then(|a| {
                if a.key.as_str() == "as" {
                    Some(a.value.as_str()?)
                } else {
                    None
                }
            })
        });

        let imported_name = as_name.unwrap_or(name.as_str());
        if self.jobs.contains_key(imported_name) {
            return self
                .origin
                .error("job name already exists", &block.labels[0].span())
                .err();
        }

        let mut job = imported
            .jobs
            .get(name.as_str())
            .ok_or_else(|| {
                self.origin.error(
                    "job name can't be imported, it doesn't exit in the imported runbook",
                    &block.labels[0].span(),
                )
            })?
            .clone();
        job.imported = Some(imported.origin.path.clone());

        self.jobs.insert(imported_name.to_string(), job.to_owned());

        Ok(())
    }

    fn hosts_from_name(&self, name: &str) -> Result<Vec<Node>> {
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

    fn parse_use_group(&mut self, imported: &Runbook, block: &Block) -> Result<(), Error> {
        if block.labels.is_empty() {
            return self
                .origin
                .error("use group needs a group name", &block.ident.span())
                .err();
        }
        if block.labels.len() > 1 {
            return self
                .origin
                .error("You can only use one group name", &block.labels[1].span())
                .err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return self
                .origin
                .error("group name should be a string", &block.labels[0].span())
                .err();
        };

        let as_name = block.body.iter().find_map(|s| {
            s.as_attribute().and_then(|a| {
                if a.key.as_str() == "as" {
                    Some(a.value.as_str()?)
                } else {
                    None
                }
            })
        });

        let imported_name = as_name.unwrap_or(name.as_str());
        if self.groups.contains_key(imported_name) {
            return self
                .origin
                .error("group name already exists", &block.labels[0].span())
                .err();
        }

        let mut group = imported
            .groups
            .get(name.as_str())
            .ok_or_else(|| {
                self.origin.error(
                    "group name can't be imported, it doesn't exit in the imported runbook",
                    &block.labels[0].span(),
                )
            })?
            .clone();
        group.imported = Some(imported.origin.path.clone());

        self.groups.insert(imported_name.to_string(), group);

        Ok(())
    }

    fn hosts_from_group(&self, group: &str) -> Result<Vec<Node>> {
        let Some(group) = self.groups.get(group) else {
            return Err(anyhow!("hosts doesn't have group {group}"));
        };

        let runbook = if let Some(imported) = &group.imported {
            self.imports
                .get(imported)
                .ok_or_else(|| anyhow!("can't find imported"))?
        } else {
            self
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
                    let mut local_hosts = runbook.hosts_from_group(group)?;
                    for host in local_hosts.iter_mut() {
                        for (key, val) in &host_or_group.vars {
                            if !host.vars.contains_key(key) {
                                if key == "remote_user" && host.remote_user.is_none() {
                                    host.remote_user = if let hcl::Value::String(s) = val {
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
                            host.remote_user = if let hcl::Value::String(s) = val {
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

    fn parse_job(&mut self, block: &Block) -> Result<(), Error> {
        if block.labels.is_empty() {
            return Error::new("job needs a name").err();
        }
        if block.labels.len() > 1 {
            return Error::new("You can only have one job name").err();
        }
        let BlockLabel::String(name) = &block.labels[0] else {
            return Error::new("job name should be a string").err();
        };

        if self.jobs.contains_key(name.as_str()) {
            return Error::new("job name already exists").err();
        }

        self.jobs.insert(
            name.to_string(),
            Job {
                block: block.to_owned(),
                imported: None,
            },
        );

        Ok(())
    }

    pub fn parse_actions(&self, ctx: &Context, block: &Block) -> Result<Vec<ActionData>, Error> {
        let all_actions = all_actions();

        let mut actions = Vec::new();
        for s in block.body.iter() {
            if let Structure::Block(block) = s {
                if block.ident.as_str() == "action" {
                    if block.labels.is_empty() {
                        return self
                            .origin
                            .error("No action name", &block.ident.span())
                            .err();
                    }
                    if block.labels.len() > 1 {
                        return self
                            .origin
                            .error("You can only have one action name", &block.labels[1].span())
                            .err();
                    }
                    let BlockLabel::String(action_name) = &block.labels[0] else {
                        return self
                            .origin
                            .error("action name should be a string", &block.labels[0].span())
                            .err();
                    };

                    let params = block.body.iter().find_map(|s| {
                        s.as_block()
                            .filter(|&block| block.ident.as_str() == "params")
                    });

                    let name = block.body.iter().find_map(|s| {
                        s.as_attribute()
                            .filter(|a| a.key.as_str() == "name")
                            .map(|a| &a.value)
                    });
                    let name = if let Some(name) = name {
                        let name =
                            SpannedValue::from_expression(&self.origin, ctx, name.to_owned())?;
                        let SpannedValue::String(s) = name else {
                            return self
                                .origin
                                .error("name should be a string", name.span())
                                .err();
                        };
                        Some(s.value().to_string())
                    } else {
                        None
                    };

                    let params = params.ok_or_else(|| {
                        self.origin
                            .error("action doesn't have params", &block.ident.span())
                    })?;

                    let mut attrs = HashMap::new();
                    for s in params.body.iter() {
                        if let Some(a) = s.as_attribute() {
                            let v = SpannedValue::from_expression(
                                &self.origin,
                                ctx,
                                a.value.to_owned(),
                            )?;
                            attrs.insert(a.key.to_string(), v);
                        }
                    }

                    if action_name.as_str() == "job" {
                        let job_name = attrs.get("name").ok_or_else(|| {
                            self.origin
                                .error("job doesn't have name in params", &params.ident.span())
                        })?;
                        let SpannedValue::String(job_name) = job_name else {
                            return self
                                .origin
                                .error("job name should be a string", job_name.span())
                                .err();
                        };
                        let job = self.jobs.get(job_name.value()).ok_or_else(|| {
                            self.origin.error("can't find job name", job_name.span())
                        })?;

                        let runbook = if let Some(imported) = &job.imported {
                            self.imports.get(imported).ok_or_else(|| {
                                self.origin
                                    .error("can't find imported job", job_name.span())
                            })?
                        } else {
                            self
                        };

                        actions.append(&mut runbook.parse_actions(ctx, &job.block)?);
                    } else {
                        let Some(action) = all_actions.get(action_name.as_str()) else {
                            return self
                                .origin
                                .error(
                                    format!("action {} can't be found", action_name.as_str()),
                                    &block.labels[0].span(),
                                )
                                .err();
                        };

                        let params =
                            action
                                .doc()
                                .parse_attrs(&self.origin, &attrs)
                                .map_err(|e| {
                                    let mut e = e;
                                    if e.location.is_none() {
                                        e = e.with_origin(&self.origin, &params.ident.span());
                                    }
                                    e
                                })?;
                        let input = action.input(params)?;
                        actions.push(ActionData {
                            id: ActionId::new(),
                            name: name.unwrap_or_else(|| action_name.to_string()),
                            action: action_name.to_string(),
                            input,
                        });
                    }
                }
            }
        }
        Ok(actions)
    }
}
