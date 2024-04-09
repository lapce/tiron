use std::collections::HashMap;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use rcl::runtime::Value;
use tiron_common::{
    action::{ActionData, ActionMessage},
    node::NodeMessage,
};
use tiron_tui::event::AppEvent;
use uuid::Uuid;

use crate::{
    local::start_local,
    remote::{start_remote, SshHost, SshRemote},
};

#[derive(Clone)]
pub struct Node {
    pub id: Uuid,
    pub host: String,
    pub remote_user: Option<String>,
    pub vars: HashMap<String, Value>,
    pub actions: Vec<ActionData>,
    pub tx: Sender<AppEvent>,
}

impl Node {
    pub fn new(host: String, vars: HashMap<String, Value>, tx: &Sender<AppEvent>) -> Self {
        Self {
            id: Uuid::new_v4(),
            host,
            remote_user: vars.get("remote_user").and_then(|v| {
                if let Value::String(s, _) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            }),
            vars,
            actions: Vec::new(),
            tx: tx.clone(),
        }
    }

    pub fn execute(&self, run_id: Uuid, exit_tx: Sender<bool>) -> Result<()> {
        let (tx, rx) = match self.start() {
            Ok((tx, rx)) => (tx, rx),
            Err(e) => {
                self.tx.send(AppEvent::Action {
                    run: run_id,
                    host: self.id,
                    msg: ActionMessage::NodeStartFailed {
                        reason: e.to_string(),
                    },
                })?;
                return Err(e);
            }
        };

        {
            let tx = self.tx.clone();
            let host_id = self.id;
            std::thread::spawn(move || {
                while let Ok(msg) = rx.recv() {
                    if let ActionMessage::NodeShutdown { success } = &msg {
                        let success = *success;
                        let _ = tx.send(AppEvent::Action {
                            run: run_id,
                            host: host_id,
                            msg,
                        });
                        let _ = exit_tx.send(success);
                        return;
                    }
                    let _ = tx.send(AppEvent::Action {
                        run: run_id,
                        host: host_id,
                        msg,
                    });
                }
                let _ = exit_tx.send(false);
            });
        }

        for action_data in &self.actions {
            tx.send(NodeMessage::Action(action_data.clone()))?;
        }
        tx.send(NodeMessage::Shutdown)?;

        Ok(())
    }

    fn start(&self) -> Result<(Sender<NodeMessage>, Receiver<ActionMessage>)> {
        if self.host == "localhost" || self.host == "127.0.0.1" {
            Ok(start_local())
        } else {
            start_remote(SshRemote {
                ssh: SshHost {
                    host: self.host.clone(),
                    port: None,
                    user: self.remote_user.clone(),
                },
            })
        }
    }
}
