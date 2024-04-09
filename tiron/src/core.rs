use std::{io::Write, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use crossbeam_channel::Sender;
use rcl::{
    ast::{Expr, Seq, Yield},
    error::IntoError,
    loader::Loader,
    markup::{MarkupMode, MarkupString},
    pprint::{self, Doc},
};
use tiron_common::error::Error;
use tiron_tui::event::{AppEvent, RunEvent};

use crate::{cli::Cli, config::Config, node::Node, run::Run};

pub fn run() {
    let cli = Cli::parse();
    let mut loader = rcl::loader::Loader::new();
    if let Err(e) = start(&cli, &mut loader) {
        print_fatal_error(e, &loader);
    }
}

fn print_fatal_error(err: Error, loader: &Loader) -> ! {
    let inputs = loader.as_inputs();
    let err = if let Some(span) = err.span {
        span.error(err.msg)
    } else {
        rcl::error::Error::new(err.msg)
    };
    let err_doc = err.report(&inputs);
    print_doc_stderr(err_doc);
    // Regardless of whether printing to stderr failed or not, the error was
    // fatal, so we exit with code 1.
    std::process::exit(1);
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

pub fn start(cli: &Cli, loader: &mut Loader) -> Result<(), Error> {
    let mut app = tiron_tui::app::App::new();
    let config = Config::load(loader, &app.tx)?;

    let runbooks = if cli.runbooks.is_empty() {
        vec!["main".to_string()]
    } else {
        cli.runbooks.clone()
    };

    let runs: Result<Vec<Vec<Run>>, Error> = runbooks
        .iter()
        .map(|name| parse_runbook(loader, name, &config, &app.tx))
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

    app.start().map_err(|e| Error::new(e.to_string(), None))?;

    Ok(())
}

fn parse_runbook(
    loader: &mut Loader,
    name: &str,
    config: &Config,
    tx: &Sender<AppEvent>,
) -> Result<Vec<Run>, Error> {
    let file_name = if !name.ends_with(".rcl") {
        format!("{name}.rcl")
    } else {
        name.to_string()
    };

    let path = match std::env::current_dir() {
        Ok(path) => path.join(file_name),
        Err(_) => PathBuf::from(file_name),
    };
    let cwd = path.parent().ok_or_else(|| {
        Error::new(
            format!("can't find parent for {}", path.to_string_lossy()),
            None,
        )
    })?;

    let data = std::fs::read_to_string(&path).map_err(|e| {
        Error::new(
            format!("can't read runbook {} error: {e}", path.to_string_lossy()),
            None,
        )
    })?;

    let id = loader.load_string(data.clone());

    let ast = loader
        .get_unchecked_ast(id)
        .map_err(|e| Error::new("", e.origin))?;

    let mut runs = Vec::new();
    let Expr::BracketLit { elements, open } = ast else {
        return Error::new("runbook should be a list", None).err();
    };
    for seq in elements {
        let mut hosts: Vec<Node> = Vec::new();
        let mut name: Option<String> = None;

        let Seq::Yield(Yield::Elem { value, span }) = seq else {
            return Error::new("run should be a dict", Some(open)).err();
        };
        let Expr::BraceLit { elements, .. } = *value else {
            return Error::new("run should be a dict", Some(span)).err();
        };

        for seq in elements {
            if let Seq::Yield(Yield::Assoc {
                key,
                value,
                value_span,
                ..
            }) = seq
            {
                if let Expr::StringLit(s, _) = *key {
                    if s.as_ref() == "hosts" {
                        if let Expr::StringLit(s, span) = *value {
                            for node in config
                                .hosts_from_name(s.as_ref())
                                .map_err(|e| Error::new(e.to_string(), span))?
                            {
                                if !hosts.iter().any(|n| n.host == node.host) {
                                    hosts.push(node);
                                }
                            }
                        } else if let Expr::BracketLit { elements, open } = *value {
                            for seq in elements {
                                let Seq::Yield(Yield::Elem { value, .. }) = seq else {
                                    return Error::new(
                                        "hosts should be list of strings",
                                        Some(open),
                                    )
                                    .err();
                                };
                                let Expr::StringLit(s, span) = *value else {
                                    return Error::new(
                                        "hosts should be list of strings",
                                        Some(open),
                                    )
                                    .err();
                                };
                                for node in config
                                    .hosts_from_name(s.as_ref())
                                    .map_err(|e| Error::new(e.to_string(), span))?
                                {
                                    if !hosts.iter().any(|n| n.host == node.host) {
                                        hosts.push(node);
                                    }
                                }
                            }
                        }
                    } else if s.as_ref() == "name" {
                        let Expr::StringLit(s, _) = *value else {
                            return Error::new("run name should be a string", Some(value_span))
                                .err();
                        };
                        name = Some(s.to_string());
                    }
                }
            }
        }
        let run = Run::from_runbook(
            loader,
            cwd,
            name,
            &data[span.start()..span.end()],
            hosts,
            tx,
        )?;
        runs.push(run);
    }

    Ok(runs)
}
