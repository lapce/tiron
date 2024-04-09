use anyhow::anyhow;
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use rcl::runtime::Value;
use serde::{Deserialize, Serialize};
use tiron_common::{
    action::{ActionId, ActionMessage},
    error::Error,
};

use super::{
    command::run_command, Action, ActionDoc, ActionParamBaseType, ActionParamDoc, ActionParamType,
};

/// Install packages
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct PackageAction {
    /// the name of the packages to be installed
    name: Vec<String>,
}

impl Action for PackageAction {
    fn name(&self) -> String {
        "package".to_string()
    }

    fn input(
        &self,
        _cwd: &std::path::Path,
        params: Option<&rcl::runtime::Value>,
    ) -> Result<Vec<u8>, Error> {
        let Some(params) = params else {
            return Error::new("can't find params", None).err();
        };
        let Value::Dict(dict, dict_span) = params else {
            return Error::new("params should be a Dict", *params.span()).err();
        };
        let Some(name) = dict.get(&Value::String("name".into(), None)) else {
            return Error::new("can't find name", *dict_span).err();
        };
        let names = match name {
            Value::String(name, _) => vec![name.to_string()],
            Value::List(name) => {
                let mut names = Vec::new();
                for name in name.iter() {
                    let Value::String(name, _) = name else {
                        return Error::new("name should be a string", *name.span()).err();
                    };
                    names.push(name.to_string());
                }
                names
            }
            _ => {
                return Error::new("name should be either a string or a list", *name.span()).err();
            }
        };

        let input = PackageAction { name: names };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}"), *params.span())
        })?;
        Ok(input)
    }

    fn execute(
        &self,
        id: ActionId,
        input: &[u8],
        tx: &Sender<ActionMessage>,
    ) -> anyhow::Result<String> {
        let input: PackageAction = bincode::deserialize(input)?;

        let mut args = vec!["install".to_string()];
        args.extend_from_slice(&input.name);

        let status = run_command(id, tx, "brew", &args)?;
        if status.success() {
            Ok("package".to_string())
        } else {
            Err(anyhow!("command failed"))
        }
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: PackageAction::DOCS.to_string(),
            params: vec![ActionParamDoc {
                name: "name".to_string(),
                required: true,
                description: PackageAction::get_field_docs("name")
                    .unwrap_or_default()
                    .to_string(),
                type_: vec![
                    ActionParamType::String,
                    ActionParamType::List(ActionParamBaseType::String),
                ],
            }],
        }
    }
}
