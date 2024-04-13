use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use hcl_edit::structure::Block;
use rcl::{error::Error, loader::Loader, markup::MarkupMode, runtime::Value, source::Span};
use tiron_common::action::ActionData;

use crate::{action::parse_actions, config::Config, run::value_to_type};

#[derive(Clone)]
pub struct Job {
    pub block: Block,
}

impl Job {
    pub fn load(
        loader: &mut Loader,
        origin: Option<Span>,
        cwd: &Path,
        name: &str,
        vars: &HashMap<String, Value>,
        job_depth: &mut i32,
        config: &Config,
    ) -> Result<Vec<ActionData>, Error> {
        let (content, path) = Self::load_file(cwd, name, config)
            .map_err(|e| Error::new(e.to_string()).with_origin(origin))?;
        let parent = path.parent().ok_or_else(|| {
            Error::new(format!(
                "job path {} doesn't have parent folder",
                path.to_string_lossy()
            ))
            .with_origin(origin)
        })?;

        let id = loader.load_string(content, Some(path.to_string_lossy().to_string()), 0);
        let mut type_env = rcl::typecheck::prelude();
        let mut env = rcl::runtime::prelude();
        for (name, value) in vars {
            type_env.push(name.as_str().into(), value_to_type(value));
            env.push(name.as_str().into(), value.clone());
        }
        let value = loader.evaluate(
            &mut type_env,
            &mut env,
            id,
            &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
        )?;

        let actions = parse_actions(loader, parent, &value, vars, job_depth, config)?;

        Ok(actions)
    }

    fn load_file(cwd: &Path, name: &str, config: &Config) -> Result<(String, PathBuf)> {
        if let Ok(content) = Self::load_file_from_folder(cwd, name) {
            return Ok(content);
        }

        if let Ok(content) = Self::load_file_from_folder(&config.project_folder.join("jobs"), name)
        {
            return Ok(content);
        }

        Err(anyhow!("can't find job {name}"))
    }

    fn load_file_from_folder(cwd: &Path, name: &str) -> Result<(String, PathBuf)> {
        {
            let path = cwd.join(name);
            if path.is_dir() {
                let path = path.join("main.rcl");
                if let Ok(content) = std::fs::read_to_string(&path) {
                    return Ok((content, path));
                }
            }
        }

        {
            let name = if name.ends_with(".rcl") {
                name.to_string()
            } else {
                format!("{name}.rcl")
            };
            let path = cwd.join(name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Ok((content, path));
            }
        }

        Err(anyhow!("can't find job {name}"))
    }
}
