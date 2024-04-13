use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use hcl_edit::structure::Block;
use rcl::{error::Error, loader::Loader, markup::MarkupMode, runtime::Value, source::Span};
use tiron_common::action::ActionData;

use crate::{config::Config, run::value_to_type};

#[derive(Clone)]
pub struct Job {
    pub block: Block,
}

impl Job {
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
