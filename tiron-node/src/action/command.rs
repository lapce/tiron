use std::{
    io::{BufRead, BufReader},
    process::{ExitStatus, Stdio},
};

use anyhow::Result;
use crossbeam_channel::Sender;
use tiron_common::action::{ActionId, ActionMessage};

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
                    let _ = tx.send(ActionMessage::ActionStdout { id, content: line });
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
                    let _ = tx.send(ActionMessage::ActionStderr { id, content: line });
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
