use std::{ops::ControlFlow, time::Duration};

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::Repository;
use ratatui::{
    DefaultTerminal, crossterm::event, layout::Rect,
};
use model::{CommitShallow, CommitDetail};

mod draw;
mod model;

struct State {
    repo: Repository,
    wanted_commit_list_count: usize,
    commits_shallow_cached: Option<Vec<CommitShallow>>,
    selected_commit_cached: Option<CommitDetail>,
    selection_idx: Option<usize>,
    diff_scroll_idx: usize,
    commits_scroll_idx: usize,
    last_log_area: Rect,
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
            selection_idx: None,
            diff_scroll_idx: 0,
            commits_scroll_idx: 0,
            last_log_area: Rect::new(0, 0, 0, 0),
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
        match event {
            event::Event::Key(key) => {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    // Quit the application using q
                    return ControlFlow::Break(());
                } else if key.code == KeyCode::Down {
                    if let Some(idx) = self.state.selection_idx {
                        self.state.selection_idx = Some(idx + 1);
                    } else {
                        self.state.selection_idx = Some(0);
                    }
                    if !self.state.last_log_area.is_empty() {
                        // Scroll down if we are at the bottom
                        let selection_idx = self.state.selection_idx.unwrap();
                        let h = self.state.last_log_area.height.saturating_sub(2);
                        if selection_idx >= self.state.commits_scroll_idx + h as usize {
                            self.state.commits_scroll_idx += 1;
                        }
                    }
                    self.state.invalidate_caches();
                } else if key.code == KeyCode::Up {
                    if let Some(idx) = self.state.selection_idx {
                        self.state.selection_idx = Some(idx.saturating_sub(1));
                    } else {
                        self.state.selection_idx = Some(0);
                    }
                    if !self.state.last_log_area.is_empty() {
                        // Scroll up if we are at the top
                        let selection_idx = self.state.selection_idx.unwrap();
                        if selection_idx < self.state.commits_scroll_idx {
                            self.state.commits_scroll_idx -= 1;
                        }
                    }
                    self.state.invalidate_caches();
                } else if key.code == KeyCode::Down {
                } else if key.code == KeyCode::Up {
                } else if key.code == KeyCode::Char('j') {
                    self.state.diff_scroll_idx += 1;
                } else if key.code == KeyCode::Char('k') {
                    self.state.diff_scroll_idx = self.state.diff_scroll_idx.saturating_sub(1);
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
