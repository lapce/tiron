use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{ExitStatus, Stdio},
};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use rcl::{error::Error, runtime::Value};
use serde::{Deserialize, Serialize};
use tiron_common::action::{ActionId, ActionMessage, ActionOutputLevel};

use super::{Action, ActionDoc, ActionParamBaseType, ActionParamDoc, ActionParamType};

pub fn run_command(
    id: ActionId,
    tx: &Sender<ActionMessage>,
    program: &str,
    args: &[String],
) -> Result<ExitStatus> {
    let mut cmd = std::process::Command::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stdout) = stdout {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line) {
                if n > 0 {
                    let line = line.trim_end().to_string();
                    let _ = tx.send(ActionMessage::ActionOutputLine {
                        id,
                        content: line,
                        level: ActionOutputLevel::Info,
                    });
                } else {
                    break;
                }
                line.clear();
            }
        });
    }

    if let Some(stderr) = stderr {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line) {
                if n > 0 {
                    let line = line.trim_end().to_string();
                    let _ = tx.send(ActionMessage::ActionOutputLine {
                        id,
                        content: line,
                        level: ActionOutputLevel::Info,
                    });
                } else {
                    break;
                }
                line.clear();
            }
        });
    }

    let status = child.wait()?;
    Ok(status)
}

/// Run the command on the remote machine
#[derive(Default, Clone, Serialize, Deserialize, Documented, DocumentedFields)]
pub struct CommandAction {
    /// The command to run
    cmd: String,
    /// The command arguments
    args: Vec<String>,
}

impl Action for CommandAction {
    fn name(&self) -> String {
        "command".to_string()
    }

    fn input(&self, _cwd: &Path, params: Option<&Value>) -> Result<Vec<u8>, Error> {
        let Some(params) = params else {
            return Error::new("can't find params").err();
        };
        let Value::Dict(dict, dict_span) = params else {
            return Error::new("params should be a Dict")
                .with_origin(*params.span())
                .err();
        };
        let Some(cmd) = dict.get(&Value::String("cmd".into(), None)) else {
            return Error::new("can't find cmd").with_origin(*dict_span).err();
        };
        let Value::String(cmd, _) = cmd else {
            return Error::new("cmd should be a string")
                .with_origin(*cmd.span())
                .err();
        };
        let args = if let Some(args) = dict.get(&Value::String("args".into(), None)) {
            let Value::List(args_value) = args else {
                return Error::new("args should be a list")
                    .with_origin(*args.span())
                    .err();
            };
            let mut args = Vec::new();
            for arg in args_value.iter() {
                let Value::String(arg, _) = arg else {
                    return Error::new("args should be a list of strings")
                        .with_origin(*arg.span())
                        .err();
                };
                args.push(arg.to_string());
            }
            Some(args)
        } else {
            None
        };
        let input = CommandAction {
            cmd: cmd.to_string(),
            args: args.unwrap_or_default(),
        };
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
        let input: CommandAction = bincode::deserialize(input)?;
        let status = run_command(id, tx, &input.cmd, &input.args)?;
        if status.success() {
            Ok("command".to_string())
        } else {
            Err(anyhow!("command failed"))
        }
    }

    fn doc(&self) -> ActionDoc {
        ActionDoc {
            description: Self::DOCS.to_string(),
            params: vec![
                ActionParamDoc {
                    name: "cmd".to_string(),
                    required: true,
                    description: Self::get_field_docs("cmd").unwrap_or_default().to_string(),
                    type_: vec![ActionParamType::String],
                },
                ActionParamDoc {
                    name: "args".to_string(),
                    required: false,
                    description: Self::get_field_docs("args").unwrap_or_default().to_string(),
                    type_: vec![ActionParamType::List(ActionParamBaseType::String)],
                },
            ],
        }
    }
}
