use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{ExitStatus, Stdio},
};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use documented::{Documented, DocumentedFields};
use serde::{Deserialize, Serialize};
use tiron_common::{
    action::{ActionId, ActionMessage, ActionOutputLevel},
    error::Error,
};

use super::{
    Action, ActionDoc, ActionParamBaseType, ActionParamDoc, ActionParamType, ActionParams,
};

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
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()?;

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

    fn input(&self, params: ActionParams) -> Result<Vec<u8>, Error> {
        let cmd = params.expect_string(0);

        let args = if let Some(list) = params.list(1) {
            let args = list
                .iter()
                .map(|v| v.expect_string().to_string())
                .collect::<Vec<_>>();
            Some(args)
        } else {
            None
        };

        let input = CommandAction {
            cmd: cmd.to_string(),
            args: args.unwrap_or_default(),
        };
        let input = bincode::serialize(&input).map_err(|e| {
            Error::new(format!("serialize action input error: {e}"))
                .with_origin(params.origin, &params.span)
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
}
