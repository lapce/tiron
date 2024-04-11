use std::{io::Write, path::Path};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use rcl::{error::Error, runtime::Value};
use serde::{Deserialize, Serialize};
use tiron_common::action::{ActionId, ActionMessage};

use super::{command::run_command, Action, ActionDoc, ActionParamDoc, ActionParamType};

/// Copy the file to the remote machine
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct CopyAction {
    /// Local path of a file to be copied
    src: String,
    content: Vec<u8>,
    /// The path where file should be copied to on remote server
    dest: String,
}

impl Action for CopyAction {
    fn name(&self) -> String {
        "copy".to_string()
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: CopyAction::DOCS.to_string(),
            params: vec![
                ActionParamDoc {
                    name: "src".to_string(),
                    required: true,
                    description: CopyAction::get_field_docs("src")
                        .unwrap_or_default()
                        .to_string(),
                    type_: vec![ActionParamType::String],
                },
                ActionParamDoc {
                    name: "dest".to_string(),
                    required: true,
                    description: CopyAction::get_field_docs("dest")
                        .unwrap_or_default()
                        .to_string(),
                    type_: vec![ActionParamType::String],
                },
            ],
        }
    }

    fn input(&self, cwd: &Path, params: Option<&Value>) -> Result<Vec<u8>, Error> {
        let Some(value) = params else {
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = value else {
            return Error::new("params should be a Dict")
                .with_origin(*value.span())
                .err();
        };
        let Some(src) = dict.get(&Value::String("src".into(), None)) else {
            return Error::new("can't find src").with_origin(*dict_span).err();
        };
        let Value::String(src, src_span) = src else {
            return Error::new("src isn't string")
                .with_origin(*src.span())
                .err();
        };
        let src_file = cwd.join(src.as_ref());
        let meta = src_file
            .metadata()
            .map_err(|_| Error::new("can't find src file").with_origin(*src_span))?;
        if !meta.is_file() {
            return Error::new("src isn't a file").with_origin(*src_span).err();
        }
        let content = std::fs::read(&src_file)
            .map_err(|e| Error::new(format!("read src file error: {e}")).with_origin(*src_span))?;

        let Some(dest) = dict.get(&Value::String("dest".into(), None)) else {
            return Error::new("can't find dest").with_origin(*dict_span).err();
        };
        let Value::String(dest, _) = dest else {
            return Error::new("dest isn't string")
                .with_origin(*dest.span())
                .err();
        };

        let input = CopyAction {
            src: src_file.to_string_lossy().to_string(),
            content,
            dest: dest.to_string(),
        };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(*value.span())
        })?;

        Ok(input)
    }

    fn execute(&self, id: ActionId, bytes: &[u8], tx: &Sender<ActionMessage>) -> Result<String> {
        let input: CopyAction = bincode::deserialize(bytes)?;
        let mut temp = tempfile::NamedTempFile::new()?;
        temp.write_all(&input.content)?;
        temp.flush()?;
        let status = run_command(
            id,
            tx,
            "cp",
            &[
                temp.path().to_string_lossy().to_string(),
                input.dest.clone(),
            ],
        )?;
        if status.success() {
            Ok(format!("copy to {}", input.dest))
        } else {
            Err(anyhow!("can't copy to {}", input.dest))
        }
    }
}
