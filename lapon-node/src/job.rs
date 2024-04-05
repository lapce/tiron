use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use lapon_common::action::ActionData;
use rcl::markup::MarkupMode;

use crate::action::data;

pub struct Job {}

impl Job {
    pub fn load(cwd: &Path, name: &str) -> Result<Vec<ActionData>> {
        let (content, path) = Self::load_file(cwd, name)?;
        let parent = path.parent().ok_or_else(|| {
            anyhow!(
                "job path {} doesn't have parent folder",
                path.to_string_lossy()
            )
        })?;

        let mut loader = rcl::loader::Loader::new();
        let id = loader.load_string(content);
        let value = loader
            .evaluate(
                &mut rcl::typecheck::prelude(),
                &mut rcl::runtime::prelude(),
                id,
                &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
            )
            .map_err(|e| {
                anyhow!(
                    "can't parse rcl file {}: {:?} {:?} {:?}",
                    path.to_string_lossy(),
                    e.message,
                    e.body,
                    e.origin
                )
            })?;

        let actions = data::parse_value(parent, &value)?;

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
