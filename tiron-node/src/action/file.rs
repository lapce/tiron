use std::path::PathBuf;

use documented::{Documented, DocumentedFields};
use rcl::{error::Error, runtime::Value};
use serde::{Deserialize, Serialize};

use super::{Action, ActionDoc, ActionParamDoc, ActionParamType};

#[derive(Default, Clone, Serialize, Deserialize)]
pub enum FileState {
    #[default]
    File,
    Directory,
    Absent,
}

/// Manage files/folders and their properties
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct FileAction {
    /// Path of the file or folder that's managed
    path: String,
    /// Default to file
    ///
    /// If `file`, a file will be managed.
    ///
    /// If `directory`, a directory will be recursively created
    /// and all of its parent components if they are missing.
    ///
    /// If `absent`, directories will be recursively deleted
    /// and all its contents, and files or symlinks will be unlinked.
    state: FileState,
}

impl Action for FileAction {
    fn name(&self) -> String {
        "file".to_string()
    }

    fn input(&self, _cwd: &std::path::Path, params: Option<&Value>) -> Result<Vec<u8>, Error> {
        let Some(params) = params else {
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = params else {
            return Error::new("params should be a Dict")
                .with_origin(*params.span())
                .err();
        };
        let Some(path) = dict.get(&Value::String("path".into(), None)) else {
            return Error::new("can't find path").with_origin(*dict_span).err();
        };
        let Value::String(path, _) = path else {
            return Error::new("path should be a string")
                .with_origin(*path.span())
                .err();
        };
        let mut input = FileAction {
            path: path.to_string(),
            ..Default::default()
        };

        if let Some(state) = dict.get(&Value::String("state".into(), None)) {
            let Value::String(state, state_span) = state else {
                return Error::new("state should be a string")
                    .with_origin(*state.span())
                    .err();
            };
            let state = match state.as_ref() {
                "file" => FileState::File,
                "absent" => FileState::Absent,
                "directory" => FileState::Directory,
                _ => {
                    return Error::new("state is invalid")
                        .with_origin(*state_span)
                        .err()
                }
            };
            input.state = state;
        };

        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(*params.span())
        })?;
        Ok(input)
    }

    fn execute(
        &self,
        _id: tiron_common::action::ActionId,
        input: &[u8],
        _tx: &crossbeam_channel::Sender<tiron_common::action::ActionMessage>,
    ) -> anyhow::Result<String> {
        let input: FileAction = bincode::deserialize(input)?;
        match input.state {
            FileState::File => {}
            FileState::Directory => {
                std::fs::create_dir_all(input.path)?;
            }
            FileState::Absent => {
                let path = PathBuf::from(input.path);
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(path)?;
                    } else {
                        std::fs::remove_file(path)?;
                    }
                }
            }
        }
        Ok("".to_string())
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: Self::DOCS.to_string(),
            params: vec![
                ActionParamDoc {
                    name: "path".to_string(),
                    required: true,
                    description: Self::get_field_docs("path").unwrap_or_default().to_string(),
                    type_: vec![ActionParamType::String],
                },
                ActionParamDoc {
                    name: "state".to_string(),
                    required: false,
                    description: Self::get_field_docs("state")
                        .unwrap_or_default()
                        .to_string(),
                    type_: vec![ActionParamType::String],
                },
            ],
        }
    }
}
