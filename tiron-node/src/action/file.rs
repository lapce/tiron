use std::path::PathBuf;

use documented::{Documented, DocumentedFields};
use rcl::error::Error;
use serde::{Deserialize, Serialize};

use super::{
    Action, ActionDoc, ActionParamBaseValue, ActionParamDoc, ActionParamType, ActionParams,
};

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
                    type_: vec![ActionParamType::Enum(vec![
                        ActionParamBaseValue::String("file".to_string()),
                        ActionParamBaseValue::String("absent".to_string()),
                        ActionParamBaseValue::String("directory".to_string()),
                    ])],
                },
            ],
        }
    }

    fn input(&self, _cwd: &std::path::Path, params: ActionParams) -> Result<Vec<u8>, Error> {
        let path = params.expect_string(0);
        let mut input = FileAction {
            path: path.to_string(),
            ..Default::default()
        };

        if let Some(state) = params.base(1) {
            let state = state.expect_string();
            let state = match state {
                "file" => FileState::File,
                "absent" => FileState::Absent,
                "directory" => FileState::Directory,
                _ => unreachable!(),
            };
            input.state = state;
        }

        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(params.span)
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
}
