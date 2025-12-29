use std::time::Duration;

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::Repository;
use ratatui::{
    DefaultTerminal, Frame, crossterm::event, text::Line, widgets::Wrap, widgets::Paragraph,
};

struct State {
    repo: Repository,
}

struct App {
    state: State,
    terminal: DefaultTerminal,
}

impl State {
    fn new() -> Result<State, anyhow::Error> {
        let state = State {
            repo: gix::open(".")?,
        };
        Ok(state)
    }
    fn draw(&self, frame: &mut Frame) -> Result<(), std::io::Error> {
        let lines = self.commits_lines()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, frame.area());
        Ok(())
    }
    fn commits_lines(&self) -> Result<Vec<Line<'_>>, anyhow::Error> {
        let head_commit = self.repo.head_commit()?;
        let msg = head_commit.message()?;
        let id = head_commit.id().shorten_or_id();
        let title = msg.title.to_string();
        let mut res = Vec::new();
        res.push(Line::from(format!("{} {}", id, title.trim())));

        let budget = 10;
        let mut commit = head_commit;

        for _ in 0..budget {
            // TODO support multiple parent IDs
            let Some(parent_id) = commit.parent_ids().next() else {
                // No parent left
                break;
            };
            commit = self.repo.find_commit(parent_id)?;
            let msg = commit.message()?;
            let id = commit.id().shorten_or_id();
            let title = msg.title.to_string();
            res.push(Line::from(format!("{} {}", id, title.trim())));
        }
        Ok(res)
    }
}

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
            if event::poll(Duration::from_millis(100)).context("failed to poll for events")? {
                match event::read().context("failed to read event")? {
                    event::Event::Key(key) => {
                        if key.code == KeyCode::Char('q') {
                            // Quit the application using q
                            break;
                        }
                    }
                    event::Event::FocusGained => (),
                    event::Event::FocusLost => (),
                    event::Event::Mouse(_) => (),
                    event::Event::Paste(_) => (),
                    event::Event::Resize(_, _) => (),
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
