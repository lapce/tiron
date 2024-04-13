mod provider;

use anyhow::anyhow;
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use rcl::error::Error;
use serde::{Deserialize, Serialize};
use tiron_common::action::{ActionId, ActionMessage};

use self::provider::PackageProvider;

use super::{
    Action, ActionDoc, ActionParamBaseType, ActionParamBaseValue, ActionParamDoc, ActionParamType,
    ActionParams,
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

    fn input(&self, _cwd: &std::path::Path, params: ActionParams) -> Result<Vec<u8>, Error> {
        let name = params.values[0].as_ref().unwrap();
        let names = if let Some(s) = name.string() {
            vec![s.to_string()]
        } else {
            let list = name.expect_list();
            list.iter().map(|v| v.expect_string().to_string()).collect()
        };

        let state = params.expect_base(1);
        let state = state.expect_string();
        let state = match state {
            "present" => PackageState::Present,
            "absent" => PackageState::Absent,
            "latest" => PackageState::Latest,
            _ => {
                unreachable!();
            }
        };

        let input = PackageAction { name: names, state };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}")).with_origin(params.span)
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
}
