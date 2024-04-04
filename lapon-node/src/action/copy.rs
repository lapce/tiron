use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use rcl::runtime::Value;
use serde::{Deserialize, Serialize};

use super::Action;

#[derive(Clone, Serialize, Deserialize)]
pub struct CopyActionInput {
    src: String,
    content: Vec<u8>,
    dest: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CopyAction {}

impl Action for CopyAction {
    fn input(&self, cwd: &Path, value: &Value) -> Result<Vec<u8>> {
        let Value::Dict(dict) = value else {
            return Err(anyhow!("input should be a Dict"));
        };
        let Some(src) = dict.get(&Value::String("src".into())) else {
            return Err(anyhow!("can't find src"));
        };
        let Value::String(src) = src else {
            return Err(anyhow!("src isn't string"));
        };
        let src_file = cwd.join(src.as_ref());
        let meta = src_file
            .metadata()
            .map_err(|_| anyhow!("can't find src file {src}"))?;
        if !meta.is_file() {
            return Err(anyhow!("src {src} isn't a file"));
        }
        let content =
            std::fs::read(&src_file).with_context(|| format!("can't read src {src} content"))?;

        let Some(dest) = dict.get(&Value::String("dest".into())) else {
            return Err(anyhow!("can't find dest"));
        };
        let Value::String(dest) = dest else {
            return Err(anyhow!("dest isn't string"));
        };

        let input = CopyActionInput {
            src: src_file.to_string_lossy().to_string(),
            content,
            dest: dest.to_string(),
        };
        let input = bincode::serialize(&input)?;

        Ok(input)
    }

    fn execute(&self, bytes: &[u8]) -> Result<String> {
        let input: CopyActionInput = bincode::deserialize(bytes)?;
        let dest = PathBuf::from(&input.dest);
        std::fs::write(dest, input.content)
            .with_context(|| format!("can't copy to dest {}", input.dest))?;
        Ok(format!("copy to {}", input.dest))
    }
}
