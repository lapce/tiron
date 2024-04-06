use anyhow::{anyhow, Result};
use crossbeam_channel::{Receiver, Sender};
use lapon_common::action::ActionMessage;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Stylize},
    widgets::{Block, Borders, List, Widget},
    Frame,
};
use uuid::Uuid;

use crate::{
    event::{AppEvent, RunEvent, UserInputEvent},
    run::RunPanel,
    tui,
};

pub struct App {
    exit: bool,
    pub runs: Vec<RunPanel>,
    // the run panel that's currently in focus
    pub focus: usize,
    pub tx: Sender<AppEvent>,
    rx: Receiver<AppEvent>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            exit: false,
            runs: Vec::new(),
            focus: 0,
            tx,
            rx,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let mut terminal = tui::init()?;
        self.run(&mut terminal)?;
        tui::restore()?;
        Ok(())
    }

    fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tui::handle_events(tx);
        });
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        match self.rx.recv()? {
            AppEvent::UserInput(event) => {
                self.handle_user_input(event)?;
            }
            AppEvent::Action { run, host, msg } => {
                self.handle_action_event(run, host, msg)?;
            }
            AppEvent::Run(event) => {
                self.handle_run_event(event)?;
            }
        };
        Ok(())
    }

    fn handle_user_input(&mut self, event: UserInputEvent) -> Result<()> {
        match event {
            UserInputEvent::ScrollUp => {}
            UserInputEvent::ScrollDown => {}
            UserInputEvent::PrevRun => {
                if self.focus == 0 {
                    self.focus = self.runs.len().saturating_sub(1);
                } else {
                    self.focus -= 1;
                }
            }
            UserInputEvent::NextRun => {
                if self.focus == self.runs.len().saturating_sub(1) {
                    self.focus = 0;
                } else {
                    self.focus += 1;
                }
            }
            UserInputEvent::Quit => self.exit(),
        }
        Ok(())
    }

    fn handle_action_event(&mut self, run: Uuid, host: Uuid, msg: ActionMessage) -> Result<()> {
        let run = self
            .runs
            .iter_mut()
            .rev()
            .find(|p| p.id == run)
            .ok_or_else(|| anyhow!("can't find run"))?;
        let host = run
            .hosts
            .iter_mut()
            .rev()
            .find(|h| h.id == host)
            .ok_or_else(|| anyhow!("can't find host"))?;
        match msg {
            ActionMessage::ActionStarted { id } => {
                let action = host.get_action(id)?;
                action.started();
            }
            ActionMessage::ActionStdout { id, content } => {
                let action = host.get_action(id)?;
                action.stdout(content);
            }
            ActionMessage::ActionStderr { id, content } => {
                let action = host.get_action(id)?;
                action.stderr(content);
            }
            ActionMessage::ActionResult { id, success } => {
                let action = host.get_action(id)?;
                action.success(success);
            }
            ActionMessage::NodeShutdown { success } => {
                host.success = Some(success);
            }
        }
        Ok(())
    }

    fn handle_run_event(&mut self, event: RunEvent) -> Result<()> {
        match event {
            RunEvent::RunStarted { id } => {
                let (i, run) = self.get_run(id)?;
                run.started = true;
                self.focus = i;
            }
            RunEvent::RunCompleted { id, success } => {
                let (_, run) = self.get_run(id)?;
                run.success = Some(success);
            }
        }
        Ok(())
    }

    fn get_run(&mut self, id: Uuid) -> Result<(usize, &mut RunPanel)> {
        let run = self
            .runs
            .iter_mut()
            .enumerate()
            .rev()
            .find(|(_, p)| p.id == id)
            .ok_or_else(|| anyhow!("can't find run"))?;
        Ok(run)
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(20),
                Constraint::Fill(1),
                Constraint::Length(20),
            ])
            .split(area);

        if let Some(run) = self
            .runs
            .get(self.focus.min(self.runs.len().saturating_sub(1)))
        {
            run.render(layout[1], buf);
            List::new(run.hosts.iter().map(|host| {
                let color = host
                    .success
                    .map(|success| if success { Color::Green } else { Color::Red });
                if let Some(color) = color {
                    host.host.clone().fg(color)
                } else {
                    host.host.clone().into()
                }
            }))
            .block(Block::default().borders(Borders::RIGHT))
            .render(layout[0], buf);
        }
        List::new(self.runs.iter().enumerate().map(|(i, run)| {
            let name = run.name.clone().unwrap_or_else(|| format!("Run {}", i + 1));

            let color = if let Some(success) = run.success {
                Some(if success { Color::Green } else { Color::Red })
            } else if run.started {
                None
            } else {
                Some(Color::Gray)
            };

            if let Some(color) = color {
                name.fg(color)
            } else {
                name.into()
            }
        }))
        .block(Block::default().borders(Borders::LEFT))
        .render(layout[2], buf);
    }
}
