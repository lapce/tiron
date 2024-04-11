mod provider;

use anyhow::anyhow;
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use rcl::{error::Error, runtime::Value};
use serde::{Deserialize, Serialize};
use tiron_common::action::{ActionId, ActionMessage};

use self::provider::PackageProvider;

use super::{
    Action, ActionDoc, ActionParamBaseType, ActionParamBaseValue, ActionParamDoc, ActionParamType,
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub enum PackageState {
    #[default]
    Present,
    Absent,
    Latest,
}

/// Install packages
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct PackageAction {
    /// the name of the packages to be installed
    name: Vec<String>,
    /// Whether to install or remove or update packages
    /// `present` to install
    /// `absent` to remove
    /// `latest` to update
    state: PackageState,
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
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = params else {
            return Error::new("params should be a Dict")
                .with_origin(*params.span())
                .err();
        };
        let Some(name) = dict.get(&Value::String("name".into(), None)) else {
            return Error::new("can't find name").with_origin(*dict_span).err();
        };
        let names = match name {
            Value::String(name, _) => vec![name.to_string()],
            Value::List(name) => {
                let mut names = Vec::new();
                for name in name.iter() {
                    let Value::String(name, _) = name else {
                        return Error::new("name should be a string")
                            .with_origin(*name.span())
                            .err();
                    };
                    names.push(name.to_string());
                }
                names
            }
            _ => {
                return Error::new("name should be either a string or a list")
                    .with_origin(*name.span())
                    .err();
            }
        };

        let Some(state) = dict.get(&Value::String("state".into(), None)) else {
            return Error::new("can't find state").with_origin(*dict_span).err();
        };
        let Value::String(state, state_span) = state else {
            return Error::new("state should be a string")
                .with_origin(*state.span())
                .err();
        };
        let state = match state.as_ref() {
            "present" => PackageState::Present,
            "absent" => PackageState::Absent,
            "latest" => PackageState::Latest,
            _ => {
                return Error::new("state is invalid")
                    .with_origin(*state_span)
                    .err()
            }
        };

        let input = PackageAction { name: names, state };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(*params.span())
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
        let provider = PackageProvider::detect()?;

        let status = provider.run(id, tx, input.name, input.state)?;
        if status.success() {
            Ok("package".to_string())
        } else {
            Err(anyhow!("package failed"))
        }
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: PackageAction::DOCS.to_string(),
            params: vec![
                ActionParamDoc {
                    name: "name".to_string(),
                    required: true,
                    description: PackageAction::get_field_docs("name")
                        .unwrap_or_default()
                        .to_string(),
                    type_: vec![
                        ActionParamType::String,
                        ActionParamType::List(ActionParamBaseType::String),
                    ],
                },
                ActionParamDoc {
                    name: "state".to_string(),
                    required: true,
                    description: PackageAction::get_field_docs("state")
                        .unwrap_or_default()
                        .to_string(),
                    type_: vec![ActionParamType::Enum(vec![
                        ActionParamBaseValue::String("present".to_string()),
                        ActionParamBaseValue::String("absent".to_string()),
                        ActionParamBaseValue::String("latest".to_string()),
                    ])],
                },
            ],
        }
    }
}
