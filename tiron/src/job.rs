use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use rcl::{markup::MarkupMode, runtime::Value};
use tiron_common::action::ActionData;

use crate::{action::parse_actions, run::value_to_type};

pub struct Job {}

impl Job {
    pub fn load(cwd: &Path, name: &str, vars: &HashMap<String, Value>) -> Result<Vec<ActionData>> {
        let (content, path) = Self::load_file(cwd, name)?;
        let parent = path.parent().ok_or_else(|| {
            anyhow!(
                "job path {} doesn't have parent folder",
                path.to_string_lossy()
            )
        })?;

        let mut loader = rcl::loader::Loader::new();
        let id = loader.load_string(content);
        let mut type_env = rcl::typecheck::prelude();
        let mut env = rcl::runtime::prelude();
        for (name, value) in vars {
            type_env.push(name.as_str().into(), value_to_type(value));
            env.push(name.as_str().into(), value.clone());
        }
        let value = loader
            .evaluate(
                &mut type_env,
                &mut env,
                id,
                &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
            )
            .map_err(|e| {
                anyhow!(
                    "can't parse job rcl file {}: {:?} {:?} {:?}",
                    path.to_string_lossy(),
                    e.message,
                    e.body,
                    e.origin
                )
            })?;

        let actions = parse_actions(parent, &value, vars)?;

        Ok(actions)
    }

    fn load_file(cwd: &Path, name: &str) -> Result<(String, PathBuf)> {
        if let Ok(content) = Self::load_file_from_folder(cwd, name) {
            return Ok(content);
        }

        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(content) = Self::load_file_from_folder(&cwd.join("jobs"), name) {
                return Ok(content);
            }
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
