use std::{ops::ControlFlow, time::Duration};

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::Repository;
use ratatui::{
    DefaultTerminal, crossterm::event, layout::Rect,
};
use model::CommitShallow;

use crate::{draw::RenderedDiff, model::Detail};

mod model;
mod draw;

struct State {
    repo: Repository,

    wanted_commit_list_count: usize,

    // Model caches
    commits_shallow_cached: Option<Vec<CommitShallow>>,
    selected_commit_cached: Option<Detail>,

    selection_idx: usize,
    diff_scroll_idx: usize,
    commits_scroll_idx: usize,

    last_rendered_diff: Option<RenderedDiff>,
    last_log_area: Rect,
    last_diff_area: Rect,
}

struct App {
    state: State,
    terminal: DefaultTerminal,
}

impl State {
    fn new() -> Result<State, anyhow::Error> {
        let state = State {
            repo: gix::open(".")?,
            wanted_commit_list_count: 10,
            commits_shallow_cached: None,
            selected_commit_cached: None,
            selection_idx: 0,
            diff_scroll_idx: 0,
            commits_scroll_idx: 0,
            last_rendered_diff: None,
            last_log_area: Rect::new(0, 0, 0, 0),
            last_diff_area: Rect::new(0, 0, 0, 0),
        };
        Ok(state)
    }
}

const POLL_INTERVAL: Duration = Duration::from_millis(100);

impl App {
    fn new(terminal: DefaultTerminal) -> Result<App, anyhow::Error> {
        let app = App {
            state: State::new()?,
            terminal,
        };
        Ok(app)
    }
    fn run(&mut self) -> Result<(), anyhow::Error> {
        loop {
            self.terminal.try_draw(|frame| self.state.draw(frame))?;
            if event::poll(POLL_INTERVAL).context("failed to poll for events")? {
                let event = event::read().context("failed to read event")?;
                match self.handle_event(event) {
                    ControlFlow::Break(()) => break,
                    ControlFlow::Continue(()) => (),
                }
            }
        }
        Ok(())
    }
    fn handle_event(&mut self, event: event::Event) -> ControlFlow<(), ()> {
        let log_h = self.state.last_log_area.height.saturating_sub(2);
        let diff_h = self.state.last_diff_area.height.saturating_sub(2) / 2;
        match event {
            event::Event::Key(key) => {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    // Quit the application using q
                    return ControlFlow::Break(());
                } else if key.code == KeyCode::Down || key.code == KeyCode::Char('k') {
                    // Scroll down log area
                    self.state.selection_idx += 1;

                    if !self.state.last_log_area.is_empty() {
                        // Scroll down if we are at the bottom
                        let selection_idx = self.state.selection_idx;
                        if selection_idx >= self.state.commits_scroll_idx + log_h as usize {
                            self.state.commits_scroll_idx += 1;
                        }
                    }
                    self.state.invalidate_caches();
                } else if key.code == KeyCode::Up || key.code == KeyCode::Char('i') {
                    // Scroll up log area
                    self.state.selection_idx = self.state.selection_idx.saturating_sub(1);

                    if !self.state.last_log_area.is_empty() {
                        // Scroll up if we are at the top
                        let selection_idx = self.state.selection_idx;
                        if selection_idx < self.state.commits_scroll_idx {
                            self.state.commits_scroll_idx -= 1;
                        }
                    }
                    self.state.invalidate_caches();
                } else if key.code == KeyCode::PageDown || key.code == KeyCode::Char('K') {
                    // Scroll down log area alot
                    self.state.selection_idx += log_h as usize;

                    self.state.commits_scroll_idx += log_h as usize;
                } else if key.code == KeyCode::PageUp || key.code == KeyCode::Char('I') {
                    // Scroll up log area alot
                    self.state.selection_idx = self.state.selection_idx.saturating_sub(log_h as usize);

                    self.state.commits_scroll_idx = self.state.commits_scroll_idx.saturating_sub(log_h as usize);
                } else if key.code == KeyCode::Char('l') {
                    // Scroll down commit area
                    self.state.diff_scroll_idx += 1;
                } else if key.code == KeyCode::Char('o') {
                    // Scroll up commit area alot
                    self.state.diff_scroll_idx = self.state.diff_scroll_idx.saturating_sub(1);
                } else if key.code == KeyCode::Char('L') {
                    // Scroll down commit area alot
                    self.state.diff_scroll_idx += diff_h as usize;
                } else if key.code == KeyCode::Char('O') {
                    // Scroll up commit area
                    self.state.diff_scroll_idx = self.state.diff_scroll_idx.saturating_sub(diff_h as usize);
                } else if key.code == KeyCode::Char('w') {
                    // Scroll up commit area to prev file
                    if let Some(rendered_diff) = &self.state.last_rendered_diff {
                        let mut ctr = 0;
                        let mut last_ctr = 0;
                        let sidx = self.state.diff_scroll_idx;
                        for (_line, text) in &rendered_diff.texts {
                            last_ctr = ctr;
                            let len = text.lines.len();
                            if sidx > ctr && sidx <= ctr + len {
                                self.state.diff_scroll_idx = ctr;
                                break;
                            }
                            ctr += len;
                        }
                        if sidx > ctr {
                            self.state.diff_scroll_idx = last_ctr;
                        }
                    }
                } else if key.code == KeyCode::Char('s') {
                    // Scroll down commit area to next file
                    if let Some(rendered_diff) = &self.state.last_rendered_diff {
                        let mut ctr = 0;
                        let sidx = self.state.diff_scroll_idx;
                        for (_line, text) in rendered_diff.texts.iter().rev().skip(1).rev() {
                            let len = text.lines.len();
                            if sidx >= ctr && sidx < ctr + len {
                                self.state.diff_scroll_idx = ctr + len;
                                break;
                            }
                            ctr += len;
                        }
                    }
                }
            }
            event::Event::FocusGained => (),
            event::Event::FocusLost => (),
            event::Event::Mouse(_) => (),
            event::Event::Paste(_) => (),
            event::Event::Resize(_, _) => {
                self.state.invalidate_caches();
            },
        }
        ControlFlow::Continue(())
    }
}

fn main() -> Result<(), anyhow::Error> {
    color_eyre::install()
        .map_err(|err| anyhow!("{}", color_eyre::Report::msg(err)))?;
    let terminal = ratatui::init();
    let mut app = App::new(terminal)?;
    app.run()?;
    ratatui::restore();
    Ok(())
}
