use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use itertools::Itertools;

use tiron_common::error::Error;
use tiron_node::action::data::all_actions;
use tiron_tui::event::{AppEvent, RunEvent};

use crate::{
    cli::{Cli, CliCmd},
    doc::generate_doc,
    fmt::fmt,
    run::Run,
    runbook::Runbook,
};

pub fn cmd() -> Result<(), Error> {
    let cli = Cli::parse();
    match cli.cmd {
        CliCmd::Run { runbooks } => {
            let runbooks = if runbooks.is_empty() {
                vec!["main".to_string()]
            } else {
                runbooks
            };
            run(runbooks, false)?;
        }
        CliCmd::Check { runbooks } => {
            let runbooks = if runbooks.is_empty() {
                vec!["main".to_string()]
            } else {
                runbooks
            };
            let runbooks = run(runbooks, true)?;
            println!("successfully checked");
            for runbook in runbooks {
                println!("{}", runbook.to_string_lossy());
            }
        }
        CliCmd::Fmt { targets } => {
            fmt(targets)?;
        }
        CliCmd::Action { name } => action_doc(name),
        CliCmd::GenerateDoc => {
            generate_doc().map_err(|e| Error::new(e.to_string()))?;
        }
    }
    Ok(())
}

pub fn run(runbooks: Vec<String>, check: bool) -> Result<Vec<PathBuf>, Error> {
    let mut app = tiron_tui::app::App::new();
    let runbooks: Vec<PathBuf> = runbooks
        .iter()
        .map(|name| {
            let file_name = if !name.ends_with(".tr") {
                format!("{name}.tr")
            } else {
                name.to_string()
            };

            match std::env::current_dir() {
                Ok(path) => path.join(file_name),
                Err(_) => PathBuf::from(file_name),
            }
        })
        .collect();

    let mut runs = Vec::new();
    for path in runbooks.iter() {
        let mut runbook = Runbook::new(path.to_path_buf(), app.tx.clone(), 0)?;
        runbook.parse(true)?;
        runs.push(runbook.runs);
    }
    let runs: Vec<Run> = runs.into_iter().flatten().collect();

    if !check {
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

        app.start().map_err(|e| Error::new(e.to_string()))?;
    }

    Ok(runbooks)
}

fn action_doc(name: Option<String>) {
    let actions = all_actions();
    if let Some(name) = name {
        if let Some(action) = actions.get(&name) {
            println!("{}\n", action.name());
            let doc = action.doc();
            println!("Description:");
            println!("  {}\n", doc.description);

            println!("Params:");
            doc.params.iter().for_each(|p| {
                println!("  - {}:", p.name);
                println!("    Required:    {}", p.required);
                println!(
                    "    Type:        {}",
                    p.type_.iter().map(|t| t.to_string()).join(" or ")
                );
                println!("    Description:");
                for line in p.description.split('\n') {
                    println!("      {line}");
                }
            });
        } else {
            println!("Can't find action {name}");
        }
    } else {
        println!("All Tiron Actions");
        actions
            .iter()
            .sorted_by_key(|(k, _)| k.to_string())
            .for_each(|(_, action)| {
                println!("  - {}:", action.name());
                println!("    {}", action.doc().description);
            });
    }
}
