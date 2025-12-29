use std::time::Duration;

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::Repository;
use ratatui::{
    DefaultTerminal, crossterm::event,
};
use model::CommitShallow;

mod draw;
mod model;

struct State {
    repo: Repository,
    wanted_commit_list_count: usize,
    commits_shallow_cached: Option<Vec<CommitShallow>>,
    selection_idx: Option<usize>,
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
            selection_idx: None,
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
                match event::read().context("failed to read event")? {
                    event::Event::Key(key) => {
                        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                            // Quit the application using q
                            break;
                        } else if key.code == KeyCode::Down {
                            if let Some(idx) = self.state.selection_idx {
                                self.state.selection_idx = Some(idx + 1);
                            } else {
                                self.state.selection_idx = Some(0);
                            }
                            self.state.invalidate_caches();
                        } else if key.code == KeyCode::Up {
                            if let Some(idx) = self.state.selection_idx {
                                self.state.selection_idx = Some(idx.saturating_sub(1));
                            } else {
                                self.state.selection_idx = Some(0);
                            }
                            self.state.invalidate_caches();
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
            }
        }
        Ok(())
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
