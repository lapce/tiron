use std::{
    collections::HashMap,
    io::{stdin, stdout, BufReader},
};

use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::{
    action::{all_actions, Action, ActionData},
    stdio::stdio_transport,
};

#[derive(Deserialize, Serialize)]
pub enum CoreMessage {
    ActionResult(String, String),
    NodeShutdown,
}

#[derive(Deserialize, Serialize)]
pub enum NodeMessage {
    Action(ActionData),
    Shutdown,
}

#[derive(Parser)]
#[clap(name = "lapon-node")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {}

pub fn start() -> Result<()> {
    let _ = Cli::parse();
    let (writer_tx, writer_rx) = crossbeam_channel::unbounded::<CoreMessage>();
    let (reader_tx, reader_rx) = crossbeam_channel::unbounded::<NodeMessage>();
    stdio_transport(stdout(), writer_rx, BufReader::new(stdin()), reader_tx);

    let all_actions = all_actions();
    let mut had_error = false;
    while let Ok(msg) = reader_rx.recv() {
        if had_error {
            continue;
        }
        match msg {
            NodeMessage::Action(action) => {
                match node_run_action(&all_actions, &action.name, &action.input) {
                    Ok(result) => {
                        writer_tx.send(CoreMessage::ActionResult(
                            action.name.clone(),
                            format!("successfully {result}"),
                        ))?;
                    }
                    Err(e) => {
                        writer_tx.send(CoreMessage::ActionResult(
                            action.name.clone(),
                            format!("error: {e:#}"),
                        ))?;
                        had_error = true;
                        writer_tx.send(CoreMessage::NodeShutdown)?;
                    }
                }
            }
            NodeMessage::Shutdown => {
                writer_tx.send(CoreMessage::NodeShutdown)?;
            }
        }
    }

    Ok(())
}

fn node_run_action(
    all_actions: &HashMap<String, Box<dyn Action>>,
    name: &str,
    input: &[u8],
) -> Result<String> {
    let result = if let Some(action) = all_actions.get(name) {
        action.execute(input)?
    } else {
        return Err(anyhow!("can't find action name {name}"));
    };
    Ok(result)
}
