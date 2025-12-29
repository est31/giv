use std::time::Duration;

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::{Repository};
use ratatui::{
    DefaultTerminal, Frame, crossterm::event, layout::{Constraint, Layout}, text::Line, widgets::{Block, Paragraph, Wrap}
};

struct CommitShallow {
    commit: String,
    author: String,
    time: String,
}

struct State {
    repo: Repository,
    commits_shallow_cached: Option<Vec<CommitShallow>>,
}

struct App {
    state: State,
    terminal: DefaultTerminal,
}

impl State {
    fn new() -> Result<State, anyhow::Error> {
        let state = State {
            repo: gix::open(".")?,
            commits_shallow_cached: None,
        };
        Ok(state)
    }
    fn draw(&mut self, frame: &mut Frame) -> Result<(), std::io::Error> {
        let (lines, authors, times) = self.commits_authors_times_lines()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let area = frame.area();
        let [commit_area, author_area, times_area] = Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: true });
        let block_commits = Block::bordered();
        frame.render_widget(paragraph.block(block_commits), commit_area);

        let paragraph = Paragraph::new(authors)
            .wrap(Wrap { trim: true });
        let block_author = Block::bordered();
        frame.render_widget(paragraph.block(block_author), author_area);

        let paragraph = Paragraph::new(times)
            .wrap(Wrap { trim: true });
        let block_times = Block::bordered();
        frame.render_widget(paragraph.block(block_times), times_area);
        Ok(())
    }
    fn commits_authors_times_lines(&mut self) -> Result<(Vec<Line<'_>>, Vec<Line<'_>>, Vec<Line<'_>>), anyhow::Error> {
        // cache the commits to display so that we don't do IO at each render iteration
        let commits_shallow = self.get_or_refresh_commits_shallow()?;
        let [mut lines, mut authors, mut times]: [Vec<_>; 3] = Default::default();
        for cmt in commits_shallow.iter() {
            lines.push(Line::from(cmt.commit.clone()));
            authors.push(Line::from(cmt.author.clone()));
            times.push(Line::from(cmt.time.clone()));
        }
        Ok((lines, authors, times))
    }
    fn get_or_refresh_commits_shallow(&mut self) -> Result<&[CommitShallow], anyhow::Error> {
        if self.commits_shallow_cached.is_none() {
            let format_time = |time: gix::date::Time| {
                time.format(gix::date::time::format::ISO8601)
            };
            let head_commit = self.repo.head_commit()?;
            let msg = head_commit.message()?;
            let id = head_commit.id().shorten_or_id();
            let title = msg.title.to_string();
            let mut res = Vec::new();
            res.push(CommitShallow {
                commit: format!("{} {}", id, title.trim()),
                author: format!("{}", head_commit.author()?.name).trim().to_owned(),
                time: format_time(head_commit.time()?)?
            });
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
                res.push(CommitShallow {
                    commit: format!("{} {}", id, title.trim()),
                    author: format!("{}", commit.author()?.name).trim().to_owned(),
                    time: format_time(commit.time()?)?
                });
            }
            Ok(self.commits_shallow_cached.insert(res))
        } else {
            Ok(self.commits_shallow_cached.as_ref().unwrap())
        }
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
