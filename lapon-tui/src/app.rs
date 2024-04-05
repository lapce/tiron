use anyhow::{anyhow, Result};
use crossbeam_channel::{Receiver, Sender};
use lapon_common::action::ActionMessage;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
    Frame,
};
use uuid::Uuid;

use crate::{
    event::{AppEvent, UserInputEvent},
    run::RunPanel,
    tui,
};

pub struct App {
    exit: bool,
    pub runs: Vec<RunPanel>,
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
            AppEvent::Run(_event) => {}
        };
        Ok(())
    }

    fn handle_user_input(&mut self, event: UserInputEvent) -> Result<()> {
        match event {
            UserInputEvent::ScrollUp => {}
            UserInputEvent::ScrollDown => {}
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
            ActionMessage::NodeShutdown => {}
        }
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let counter_text = Text::from(vec![Line::from(vec!["This is the first task This is the first task This is the first task This is the first task".into()])
            .style(Style::default().on_gray())]);

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(20),
                Constraint::Fill(1),
                Constraint::Length(20),
            ])
            .split(area);

        if let Some(run) = self.runs.first() {
            run.render(layout[1], buf);
        }
        Paragraph::new(counter_text.clone())
            .block(Block::default().borders(Borders::RIGHT))
            .render(layout[0], buf);
        Paragraph::new(counter_text)
            .block(Block::default().borders(Borders::LEFT))
            .render(layout[2], buf);
    }
}
