use anyhow::anyhow;
use crossbeam_channel::Sender;
use rcl::runtime::Value;
use serde::{Deserialize, Serialize};
use tiron_common::action::{ActionId, ActionMessage};

use super::{command::run_command, Action};

#[derive(Clone, Serialize, Deserialize)]
pub struct PackageActionInput {
    names: Vec<String>,
}

pub struct PackageAction;

impl Action for PackageAction {
    fn input(
        &self,
        _cwd: &std::path::Path,
        params: Option<&rcl::runtime::Value>,
    ) -> anyhow::Result<Vec<u8>> {
        let Some(params) = params else {
            return Err(anyhow!("can't find params"));
        };
        let Value::Dict(params) = params else {
            return Err(anyhow!("params should be a Dict"));
        };
        let Some(name) = params.get(&Value::String("name".into())) else {
            return Err(anyhow!("can't find name"));
        };
        let names = match name {
            Value::String(name) => vec![name.to_string()],
            Value::List(name) => {
                let mut names = Vec::new();
                for name in name.iter() {
                    let Value::String(name) = name else {
                        return Err(anyhow!("name should be a string"));
                    };
                    names.push(name.to_string());
                }
                names
            }
            _ => {
                return Err(anyhow!("name should be either a string or a list"));
            }
        };

        let input = PackageActionInput { names };
        let input = bincode::serialize(&input)?;
        Ok(input)
    }

    fn execute(
        &self,
        id: ActionId,
        input: &[u8],
        tx: &Sender<ActionMessage>,
    ) -> anyhow::Result<String> {
        let input: PackageActionInput = bincode::deserialize(input)?;

        let mut args = vec!["install".to_string()];
        args.extend_from_slice(&input.names);

        let status = run_command(id, tx, "brew", &args)?;
        if status.success() {
            Ok("package".to_string())
        } else {
            Err(anyhow!("command failed"))
        }
    }
}
