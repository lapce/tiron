use anyhow::{anyhow, Result};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    text::StyledGrapheme,
    widgets::{Block, Borders, List, ListState, StatefulWidget},
};
use tiron_common::action::{ActionId, ActionOutput, ActionOutputLevel, ActionOutputLine};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;

use crate::reflow::{LineComposer, WordWrapper, WrappedLine};

pub struct HostSection {
    pub id: Uuid,
    pub host: String,
    pub actions: Vec<ActionSection>,
    pub scroll: u16,
    pub success: Option<bool>,
}

impl HostSection {
    pub fn get_action(&mut self, id: ActionId) -> Result<&mut ActionSection> {
        let action = self
            .actions
            .iter_mut()
            .rev()
            .find(|a| a.id == id)
            .ok_or_else(|| anyhow!("can't find action"))?;
        Ok(action)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = Rect::new(
            area.left() + 1,
            area.top() + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );

        let mut y = 0;

        for action in &self.actions {
            action.render(area, buf, &mut y, self.scroll);
            y += 1;
            if y >= area.height + self.scroll {
                break;
            }
        }
    }
}

pub struct ActionSection {
    pub id: ActionId,
    pub name: String,
    pub output: ActionOutput,
    pub folded: bool,
}

impl ActionSection {
    pub fn started(&mut self) {
        self.output.started = true;
    }

    pub fn stdout(&mut self, content: String) {
        self.output.lines.push(ActionOutputLine {
            content,
            level: ActionOutputLevel::Info,
        });
    }

    pub fn stderr(&mut self, content: String) {
        self.output.lines.push(ActionOutputLine {
            content,
            level: ActionOutputLevel::Error,
        });
    }

    pub fn success(&mut self, success: bool) {
        self.output.success = Some(success);
    }
}

pub struct RunPanel {
    pub id: Uuid,
    pub name: Option<String>,
    pub active: usize,
    pub hosts: Vec<HostSection>,
    pub hosts_state: ListState,
    pub started: bool,
    pub success: Option<bool>,
}

impl RunPanel {
    pub fn new(id: Uuid, name: Option<String>, hosts: Vec<HostSection>) -> Self {
        Self {
            id,
            name,
            active: 0,
            hosts,
            hosts_state: ListState::default().with_selected(Some(0)),
            started: false,
            success: None,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if let Some(host) = self.hosts.first() {
            host.render(area, buf);
        }
    }

    pub fn render_hosts(&mut self, area: Rect, buf: &mut Buffer) {
        self.hosts_state.select(Some(self.active));
        List::new(self.hosts.iter().map(|host| {
            let color = host
                .success
                .map(|success| if success { Color::Green } else { Color::Red });
            if let Some(color) = color {
                host.host.clone().fg(color)
            } else {
                host.host.clone().into()
            }
        }))
        .highlight_symbol(" > ")
        .block(Block::default().borders(Borders::RIGHT))
        .render(area, buf, &mut self.hosts_state);
    }
}

const fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}

impl HostSection {
    pub fn new(id: Uuid, host: String, actions: Vec<ActionSection>) -> Self {
        Self {
            id,
            host,
            actions,
            scroll: 0,
            success: None,
        }
    }
}

impl ActionSection {
    pub fn new(id: ActionId, name: String) -> Self {
        Self {
            id,
            name,
            folded: false,
            output: ActionOutput::default(),
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer, y: &mut u16, scroll: u16) {
        let bg = if let Some(success) = self.output.success {
            if success {
                Color::Green
            } else {
                Color::Red
            }
        } else if self.output.started {
            Color::Yellow
        } else {
            Color::Gray
        };
        self.render_line(area, buf, y, scroll, &self.name, None, Some(bg));
        *y += 1;
        if self.folded {
            return;
        }
        if *y >= area.height + scroll {
            return;
        }
        for line in &self.output.lines {
            let fg = match line.level {
                ActionOutputLevel::Info => None,
                ActionOutputLevel::Warn => Some(Color::Yellow),
                ActionOutputLevel::Error => Some(Color::Red),
            };
            self.render_line(area, buf, y, scroll, &line.content, fg, None);
            if *y >= area.height + scroll {
                return;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line(
        &self,
        area: Rect,
        buf: &mut Buffer,
        y: &mut u16,
        scroll: u16,
        line: &str,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        let style = Style::default();
        let style = if let Some(fg) = fg {
            style.fg(fg)
        } else {
            style
        };
        let mut line_composer = WordWrapper::new(
            vec![(
                line.graphemes(true)
                    .map(move |g| StyledGrapheme { symbol: g, style }),
                Alignment::Left,
            )]
            .into_iter(),
            area.width,
            false,
        );

        while let Some(WrappedLine {
            line: current_line,
            width: current_line_width,
            alignment: current_line_alignment,
        }) = line_composer.next_line()
        {
            if *y >= scroll {
                if let Some(bg) = bg {
                    let area = Rect::new(area.left(), area.top() + *y - scroll, area.width, 1);
                    buf.set_style(area, Style::default().bg(bg));
                }
                let mut x = get_line_offset(current_line_width, area.width, current_line_alignment);
                for StyledGrapheme { symbol, style } in current_line {
                    let width = symbol.width();
                    if width == 0 {
                        continue;
                    }
                    // If the symbol is empty, the last char which rendered last time will
                    // leave on the line. It's a quick fix.
                    let symbol = if symbol.is_empty() { " " } else { symbol };
                    buf.get_mut(area.left() + x, area.top() + *y - scroll)
                        .set_symbol(symbol)
                        .set_style(*style);
                    x += width as u16;
                }
            }
            *y += 1;
            if *y >= area.height + scroll {
                break;
            }
        }
    }
}
