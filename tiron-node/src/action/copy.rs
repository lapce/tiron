use std::io::Write;

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use serde::{Deserialize, Serialize};
use tiron_common::{
    action::{ActionId, ActionMessage},
    error::Error,
};

use super::{
    command::run_command, Action, ActionDoc, ActionParamDoc, ActionParamType, ActionParams,
};

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

    fn input(&self, params: ActionParams) -> Result<Vec<u8>, Error> {
        let (src, src_span) = params.expect_string_with_span(0);
        let src_file = params.origin.cwd.join(src);
        let meta = src_file
            .metadata()
            .map_err(|_| Error::new("can't find src file").with_origin(params.origin, src_span))?;
        if !meta.is_file() {
            return Error::new("src isn't a file")
                .with_origin(params.origin, src_span)
                .err();
        }
        let content = std::fs::read(&src_file).map_err(|e| {
            Error::new(format!("read src file error: {e}")).with_origin(params.origin, src_span)
        })?;

        let dest = params.expect_string(1);

        let input = CopyAction {
            src: src_file.to_string_lossy().to_string(),
            content,
            dest: dest.to_string(),
        };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}"))
                .with_origin(params.origin, &params.span)
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
