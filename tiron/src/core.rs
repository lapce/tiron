use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::Sender;
use rcl::ast::{Expr, Seq, Yield};
use tiron_tui::event::{AppEvent, RunEvent};

use crate::{cli::Cli, config::Config, node::Node, run::Run};

pub fn start(cli: &Cli) -> Result<()> {
    let mut app = tiron_tui::app::App::new();
    let config = Config::load(&app.tx)?;

    let runbooks = if cli.runbooks.is_empty() {
        vec!["main".to_string()]
    } else {
        cli.runbooks.clone()
    };

    let runs: Result<Vec<Vec<Run>>> = runbooks
        .iter()
        .map(|name| parse_runbook(name, &config, &app.tx))
        .collect();
    let runs: Vec<Run> = runs?.into_iter().flatten().collect();

    app.runs = runs.iter().map(|run| run.to_panel()).collect();

    let tx = app.tx.clone();
    std::thread::spawn(move || -> Result<()> {
        for run in runs {
            let _ = tx.send(AppEvent::Run(RunEvent::RunStarted { id: run.id }));
            let success = run.execute()?;
            let _ = tx.send(AppEvent::Run(RunEvent::RunCompleted {
                id: run.id,
                success,
            }));
            if !success {
                break;
            }
        }
        Ok(())
    });

    app.start()?;

    Ok(())
}

fn parse_runbook(name: &str, config: &Config, tx: &Sender<AppEvent>) -> Result<Vec<Run>> {
    let file_name = if !name.ends_with(".rcl") {
        format!("{name}.rcl")
    } else {
        name.to_string()
    };

    let path = match std::env::current_dir() {
        Ok(path) => path.join(file_name),
        Err(_) => PathBuf::from(file_name),
    };
    let cwd = path
        .parent()
        .ok_or_else(|| anyhow!("can't find parent for {}", path.to_string_lossy()))?;

    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("can't reading runbook {}", path.to_string_lossy()))?;

    let mut loader = rcl::loader::Loader::new();
    let id = loader.load_string(data.clone());

    let ast = loader.get_unchecked_ast(id).map_err(|e| {
        anyhow!(
            "can't parse run book {}: {:?} {:?} {:?}",
            path.to_string_lossy(),
            e.message,
            e.body,
            e.origin
        )
    })?;

    let mut runs = Vec::new();
    let Expr::BracketLit { elements, .. } = ast else {
        return Err(anyhow!("runbook should be a list"));
    };
    for seq in elements {
        let mut hosts: Vec<Node> = Vec::new();
        let mut name: Option<String> = None;

        let Seq::Yield(Yield::Elem { value, span }) = seq else {
            return Err(anyhow!("run should be a dict"));
        };
        let Expr::BraceLit { elements, .. } = *value else {
            return Err(anyhow!("run should be a dict"));
        };

        for seq in elements {
            if let Seq::Yield(Yield::Assoc { key, value, .. }) = seq {
                if let Expr::StringLit(s) = *key {
                    if s.as_ref() == "hosts" {
                        if let Expr::StringLit(s) = *value {
                            for node in config.hosts_from_name(s.as_ref())? {
                                if !hosts.iter().any(|n| n.host == node.host) {
                                    hosts.push(node);
                                }
                            }
                        } else if let Expr::BracketLit { elements, .. } = *value {
                            for seq in elements {
                                let Seq::Yield(Yield::Elem { value, .. }) = seq else {
                                    return Err(anyhow!("hosts should be list of strings"));
                                };
                                let Expr::StringLit(s) = *value else {
                                    return Err(anyhow!("hosts should be list of strings"));
                                };
                                for node in config.hosts_from_name(s.as_ref())? {
                                    if !hosts.iter().any(|n| n.host == node.host) {
                                        hosts.push(node);
                                    }
                                }
                            }
                        }
                    } else if s.as_ref() == "name" {
                        let Expr::StringLit(s) = *value else {
                            return Err(anyhow!("run name should be a string"));
                        };
                        name = Some(s.to_string());
                    }
                }
            }
        }
        let run = Run::from_runbook(cwd, name, &data[span.start()..span.end()], hosts, tx)?;
        runs.push(run);
    }

    Ok(runs)
}
