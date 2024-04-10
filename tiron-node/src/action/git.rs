use anyhow::anyhow;
use documented::{Documented, DocumentedFields};
use rcl::{error::Error, runtime::Value};
use serde::{Deserialize, Serialize};

use super::{command::run_command, Action, ActionDoc, ActionParamDoc, ActionParamType};

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

    fn input(&self, _cwd: &std::path::Path, params: Option<&Value>) -> Result<Vec<u8>, Error> {
        let Some(params) = params else {
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = params else {
            return Error::new("params should be a Dict")
                .with_origin(*params.span())
                .err();
        };
        let Some(repo) = dict.get(&Value::String("repo".into(), None)) else {
            return Error::new("can't find repo").with_origin(*dict_span).err();
        };
        let Value::String(repo, _) = repo else {
            return Error::new("repo should be a string")
                .with_origin(*repo.span())
                .err();
        };
        let Some(dest) = dict.get(&Value::String("dest".into(), None)) else {
            return Error::new("can't find dest").with_origin(*dict_span).err();
        };
        let Value::String(dest, _) = dest else {
            return Error::new("dest should be a string")
                .with_origin(*dest.span())
                .err();
        };

        let input = GitAction {
            repo: repo.to_string(),
            dest: dest.to_string(),
        };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(*params.span())
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
