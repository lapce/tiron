use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::Parser;
use itertools::Itertools;
use rcl::{
    ast::{Expr, Seq, Yield},
    error::Error,
    loader::Loader,
    markup::{MarkupMode, MarkupString},
    pprint::{self, Doc},
};
use tiron_node::action::data::all_actions;
use tiron_tui::event::{AppEvent, RunEvent};

use crate::{
    cli::{Cli, CliCmd},
    config::Config,
    node::Node,
    run::Run,
};

pub fn cmd() {
    let cli = Cli::parse();
    match cli.cmd {
        CliCmd::Run { runbooks } => {
            let runbooks = if runbooks.is_empty() {
                vec!["main".to_string()]
            } else {
                runbooks
            };
            let mut loader = rcl::loader::Loader::new();
            if let Err(e) = run(&mut loader, runbooks, false) {
                print_fatal_error(e, &loader);
            }
        }
        CliCmd::Check { runbooks } => {
            let runbooks = if runbooks.is_empty() {
                vec!["main".to_string()]
            } else {
                runbooks
            };
            let mut loader = rcl::loader::Loader::new();
            match run(&mut loader, runbooks, true) {
                Ok(runbooks) => {
                    println!("successfully checked");
                    for runbook in runbooks {
                        println!("{}", runbook.to_string_lossy());
                    }
                }
                Err(e) => {
                    print_fatal_error(e, &loader);
                }
            }
        }
        CliCmd::Action { name } => action_doc(name),
    }
}

fn print_fatal_error(err: Error, loader: &Loader) -> ! {
    let inputs = loader.as_inputs();
    let err_doc = err.report(&inputs);
    print_doc_stderr(err_doc);
    // Regardless of whether printing to stderr failed or not, the error was
    // fatal, so we exit with code 1.
    std::process::exit(1);
}

pub fn print_warn(err: Error, loader: &Loader) {
    let inputs = loader.as_inputs();
    let err_doc = err.report(&inputs);
    print_doc_stderr(err_doc);
}

fn print_doc_stderr(doc: Doc) {
    let stderr = std::io::stderr();
    let markup = MarkupMode::Ansi;
    let cfg = pprint::Config { width: 80 };
    let result = doc.println(&cfg);
    let mut out = stderr.lock();
    print_string(markup, result, &mut out);
}

fn print_string(mode: MarkupMode, data: MarkupString, out: &mut dyn Write) {
    let res = data.write_bytes(mode, out);
    if res.is_err() {
        // If we fail to print to stdout/stderr, there is no point in
        // printing an error, just exit then.
        std::process::exit(1);
    }
}

pub fn run(loader: &mut Loader, runbooks: Vec<String>, check: bool) -> Result<Vec<PathBuf>, Error> {
    let mut app = tiron_tui::app::App::new();
    let config = Config::load(loader, &app.tx)?;

    let runbooks: Vec<PathBuf> = runbooks
        .iter()
        .map(|name| {
            let file_name = if !name.ends_with(".rcl") {
                format!("{name}.rcl")
            } else {
                name.to_string()
            };

            match std::env::current_dir() {
                Ok(path) => path.join(file_name),
                Err(_) => PathBuf::from(file_name),
            }
        })
        .collect();

    let runs: Result<Vec<Vec<Run>>, Error> = runbooks
        .iter()
        .map(|name| parse_runbook(loader, name, &config))
        .collect();
    let runs: Vec<Run> = runs?.into_iter().flatten().collect();

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

        app.start()
            .map_err(|e| Error::new(e.to_string()).with_origin(None))?;
    }

    Ok(runbooks)
}

fn parse_runbook(loader: &mut Loader, path: &Path, config: &Config) -> Result<Vec<Run>, Error> {
    let cwd = path
        .parent()
        .ok_or_else(|| Error::new(format!("can't find parent for {}", path.to_string_lossy())))?;

    let data = std::fs::read_to_string(path).map_err(|e| {
        Error::new(format!(
            "can't read runbook {} error: {e}",
            path.to_string_lossy()
        ))
    })?;

    let id = loader.load_string(data.clone(), Some(path.to_string_lossy().to_string()), 0);

    let ast = loader.get_unchecked_ast(id)?;

    let mut runs = Vec::new();
    let Expr::BracketLit { elements, open } = ast else {
        return Error::new("runbook should be a list").err();
    };
    for seq in elements {
        let mut hosts: Vec<Node> = Vec::new();
        let mut name: Option<String> = None;

        let Seq::Yield(Yield::Elem { value, span }) = seq else {
            return Error::new("run should be a dict")
                .with_origin(Some(open))
                .err();
        };
        let Expr::BraceLit { elements, .. } = *value else {
            return Error::new("run should be a dict")
                .with_origin(Some(span))
                .err();
        };

        for seq in elements {
            if let Seq::Yield(Yield::Assoc {
                key,
                value,
                value_span,
                ..
            }) = seq
            {
                if let Expr::StringLit(s, hosts_span) = *key {
                    if s.as_ref() == "hosts" {
                        if let Expr::StringLit(s, span) = *value {
                            for node in config
                                .hosts_from_name(s.as_ref())
                                .map_err(|e| Error::new(e.to_string()).with_origin(span))?
                            {
                                if !hosts.iter().any(|n| n.host == node.host) {
                                    hosts.push(node);
                                }
                            }
                        } else if let Expr::BracketLit { elements, open } = *value {
                            for seq in elements {
                                let Seq::Yield(Yield::Elem { value, span }) = seq else {
                                    return Error::new("hosts should be list of strings")
                                        .with_origin(Some(open))
                                        .err();
                                };
                                let Expr::StringLit(s, span) = *value else {
                                    return Error::new("hosts should be list of strings")
                                        .with_origin(Some(span))
                                        .err();
                                };
                                for node in config
                                    .hosts_from_name(s.as_ref())
                                    .map_err(|e| Error::new(e.to_string()).with_origin(span))?
                                {
                                    if !hosts.iter().any(|n| n.host == node.host) {
                                        hosts.push(node);
                                    }
                                }
                            }
                        } else {
                            return Error::new("hosts should be a string or list of strings")
                                .with_origin(hosts_span)
                                .err();
                        }
                    } else if s.as_ref() == "name" {
                        let Expr::StringLit(s, _) = *value else {
                            return Error::new("run name should be a string")
                                .with_origin(Some(value_span))
                                .err();
                        };
                        name = Some(s.to_string());
                    }
                }
            }
        }
        let run = Run::from_runbook(loader, cwd, name, span, hosts, config)?;
        runs.push(run);
    }

    Ok(runs)
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
