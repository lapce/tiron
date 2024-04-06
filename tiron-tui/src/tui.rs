use std::io::{stdout, Stdout};

use anyhow::Result;
use crossbeam_channel::Sender;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::event::{AppEvent, UserInputEvent};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init() -> Result<Tui> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let tui = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok(tui)
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

pub fn handle_events(tx: Sender<AppEvent>) -> Result<()> {
    while let Ok(event) = crossterm::event::read() {
        let event = match event {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                match key_event.code {
                    KeyCode::Char('q') => UserInputEvent::Quit,
                    KeyCode::Char('j') => UserInputEvent::ScrollDown,
                    KeyCode::Char('k') => UserInputEvent::ScrollUp,
                    KeyCode::Char('p') if key_event.modifiers == KeyModifiers::CONTROL => {
                        UserInputEvent::PrevRun
                    }
                    KeyCode::Char('n') if key_event.modifiers == KeyModifiers::CONTROL => {
                        UserInputEvent::NextRun
                    }
                    KeyCode::Char('p') if key_event.modifiers.is_empty() => {
                        UserInputEvent::PrevHost
                    }
                    KeyCode::Char('n') if key_event.modifiers.is_empty() => {
                        UserInputEvent::NextHost
                    }
                    _ => continue,
                }
            }
            _ => continue,
        };
        tx.send(AppEvent::UserInput(event))?;
    }
    Ok(())
}
