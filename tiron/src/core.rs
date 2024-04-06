use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::Sender;
use rcl::{markup::MarkupMode, runtime::Value};
use tiron_tui::event::{AppEvent, RunEvent};

use crate::{cli::Cli, config::Config, run::Run};

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
    let id = loader.load_string(data);
    let value = loader
        .evaluate(
            &mut rcl::typecheck::prelude(),
            &mut rcl::runtime::prelude(),
            id,
            &mut rcl::tracer::StderrTracer::new(Some(MarkupMode::Ansi)),
        )
        .map_err(|e| {
            anyhow!(
                "can't parse rcl file: {:?} {:?} {:?}",
                e.message,
                e.body,
                e.origin
            )
        })?;

    let Value::List(runs) = value else {
        return Err(anyhow!("runbook should be a list"));
    };

    let runs: Result<Vec<Run>> = runs
        .iter()
        .map(|v| Run::from_value(cwd, config, v, tx))
        .collect();
    let runs = runs?;

    Ok(runs)
}
