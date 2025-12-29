use std::time::Duration;

use anyhow::{Context, anyhow};
use crossterm::event::KeyCode;
use gix::{ObjectId, Repository};
use ratatui::{
    DefaultTerminal, Frame, crossterm::event, layout::{Constraint, Layout}, style::Stylize, text::{Line, Span}, widgets::{Block, Paragraph, Wrap}
};

struct CommitShallow {
    id: ObjectId,
    commit: String,
    author: String,
    time: String,
}

struct CommitDetail {
    commit: String,
    author: String,
    committer: String,
    title: String,
    msg_detail: String,
    diff_parent: String,
}
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
    fn draw(&mut self, frame: &mut Frame) -> Result<(), std::io::Error> {
        let area = frame.area();

        // We allocate a bit more commits here than needed but this is ok
        if self.wanted_commit_list_count != area.height as usize {
            self.wanted_commit_list_count = area.height as usize;
            self.invalidate_caches();
        }

        let (lines, authors, times) = self.commits_authors_times_lines()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let [log_area, diff_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let [commit_area, author_area, times_area] = Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1), Constraint::Fill(1)]).areas(log_area);

        let paragraph = Paragraph::new(lines);
        let block_commits = Block::bordered();
        frame.render_widget(paragraph.block(block_commits), commit_area);

        let paragraph = Paragraph::new(authors);
        let block_author = Block::bordered();
        frame.render_widget(paragraph.block(block_author), author_area);

        let paragraph = Paragraph::new(times);
        let block_times = Block::bordered();
        frame.render_widget(paragraph.block(block_times), times_area);

        if let Some(selected_commit) = self.get_selected_commit()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        {
            fn line_with_kind<'a>(kind: &'a str, s: String) -> Line<'a> {
                Line::from(vec![Span::from(kind).bold(), Span::from(s)])
            }
            let lines = vec![
                line_with_kind("Author: ", selected_commit.author),
                line_with_kind("Committer: ", selected_commit.committer),
                Line::from(""),
                Line::from(selected_commit.title),
                Line::from(""),
                Line::from(selected_commit.msg_detail),
            ];
            let paragraph = Paragraph::new(lines)
                .wrap(Wrap { trim: true });
            let block_selected = Block::bordered();
            frame.render_widget(paragraph.block(block_selected), diff_area);
        }
        Ok(())
    }
    fn commits_authors_times_lines(&mut self) -> Result<(Vec<Line<'_>>, Vec<Line<'_>>, Vec<Line<'_>>), anyhow::Error> {
        // cache the commits to display so that we don't do IO at each render iteration
        let selection_idx = self.selection_idx;
        let commits_shallow = self.get_or_refresh_commits_shallow()?;
        let [mut lines, mut authors, mut times]: [Vec<_>; 3] = Default::default();

        let selected_st = ratatui::style::Modifier::BOLD;
        for (idx, cmt) in commits_shallow.iter().enumerate() {
            if Some(idx) == selection_idx {
                lines.push(Line::from(cmt.commit.clone()).style(selected_st));
                authors.push(Line::from(cmt.author.clone()).style(selected_st));
                times.push(Line::from(cmt.time.clone()).style(selected_st));
            } else {
            lines.push(Line::from(cmt.commit.clone()));
            authors.push(Line::from(cmt.author.clone()));
            times.push(Line::from(cmt.time.clone()));
            }
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
                id: head_commit.id,
                commit: format!("{} {}", id, title.trim()),
                author: format!("{} <{}>", head_commit.author()?.name, head_commit.author()?.email).trim().to_owned(),
                time: format_time(head_commit.time()?)?
            });
            let budget = self.wanted_commit_list_count;
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
                id: commit.id,
                    commit: format!("{} {}", id, title.trim()),
                    author: format!("{} <{}>", commit.author()?.name, commit.author()?.email).trim().to_owned(),
                    time: format_time(commit.time()?)?
                });
            }
            Ok(self.commits_shallow_cached.insert(res))
        } else {
            Ok(self.commits_shallow_cached.as_ref().unwrap())
        }
    }
    fn get_selected_commit(&mut self) -> Result<Option<CommitDetail>, anyhow::Error> {
        let Some(selection_idx) = self.selection_idx else {
            return Ok(None);
        };
        let id = {
            let selected_hash = self.get_or_refresh_commits_shallow()?;
            let Some(selected_commit) = selected_hash.get(selection_idx) else {
                return Ok(None);
            };
            selected_commit.id
        };
        let commit = self.repo.find_commit(id)?;
        let msg = commit.message()?;
        let title = msg.title.to_string().trim().to_owned();
        let msg_detail = if let Some(body) = msg.body() {
            body.without_trailer().to_string()
        } else {
            String::new()
        };
        let author = format!("{} <{}>", commit.author()?.name, commit.author()?.email).trim().to_owned();
        let committer = format!("{} <{}>", commit.committer()?.name, commit.committer()?.email).trim().to_owned();
        let commit = String::new();
        let diff_parent = String::new();
        Ok(Some(CommitDetail { commit, author, committer, title, msg_detail, diff_parent }))
    }
    fn invalidate_caches(&mut self) {
        self.commits_shallow_cached = None;
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
