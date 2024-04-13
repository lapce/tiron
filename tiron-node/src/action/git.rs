use anyhow::anyhow;
use documented::{Documented, DocumentedFields};
use serde::{Deserialize, Serialize};
use tiron_common::error::Error;

use super::{
    command::run_command, Action, ActionDoc, ActionParamDoc, ActionParamType, ActionParams,
};

/// Manage Git repositories
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct GitAction {
    /// address of the git repository
    repo: String,
    /// The path of where the repository should be checked out.
    dest: String,
}

impl Action for GitAction {
    fn name(&self) -> String {
        "git".to_string()
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: Self::DOCS.to_string(),
            params: vec![
                ActionParamDoc {
                    name: "repo".to_string(),
                    required: true,
                    description: Self::get_field_docs("repo").unwrap_or_default().to_string(),
                    type_: vec![ActionParamType::String],
                },
                ActionParamDoc {
                    name: "dest".to_string(),
                    required: true,
                    description: Self::get_field_docs("dest").unwrap_or_default().to_string(),
                    type_: vec![ActionParamType::String],
                },
            ],
        }
    }

    fn input(&self, params: ActionParams) -> Result<Vec<u8>, Error> {
        let repo = params.expect_string(0);
        let dest = params.expect_string(1);

        let input = GitAction {
            repo: repo.to_string(),
            dest: dest.to_string(),
        };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}"))
                .with_origin(params.origin, &params.span)
        })?;
        Ok(input)
    }

    fn execute(
        &self,
        id: tiron_common::action::ActionId,
        input: &[u8],
        tx: &crossbeam_channel::Sender<tiron_common::action::ActionMessage>,
    ) -> anyhow::Result<String> {
        let input: GitAction = bincode::deserialize(input)?;
        let status = run_command(
            id,
            tx,
            "git",
            &["clone".to_string(), input.repo, input.dest],
        )?;
        if status.success() {
            Ok("command".to_string())
        } else {
            Err(anyhow!("command failed"))
        }
    }
}
