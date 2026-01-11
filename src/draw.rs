use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarState, Wrap},
};

use crate::model::{CommitDetail, Detail, Diff, FileModificationKind};

use super::State;

#[derive(Clone)]
pub(crate) struct RenderedDiff {
    pub(crate) texts: Vec<(Line<'static>, Text<'static>)>,
}

impl State {
    pub(crate) fn draw(&mut self, frame: &mut Frame) -> Result<(), std::io::Error> {
        let area = frame.area();

        let [log_area, diff_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(2)]).areas(area);

        // We allocate a bit more commits here than needed but this is ok
        if self.wanted_commit_list_count != log_area.height as usize + self.commits_scroll_idx {
            self.wanted_commit_list_count = log_area.height as usize + self.commits_scroll_idx;
            self.invalidate_caches();
        }

        self.last_log_area = log_area;
        self.last_diff_area = diff_area;

        self.draw_log_area(frame, log_area)?;
        self.draw_selected_commit_area(frame, diff_area)?;

        Ok(())
    }
    fn draw_log_area(&mut self, frame: &mut Frame, log_area: Rect) -> Result<(), std::io::Error> {
        let commits_scroll_idx = self.commits_scroll_idx as u16;

        let (lines, authors, times) = self
            .commits_authors_times_lines()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let [commit_area, author_area, times_area] = Layout::horizontal([
            Constraint::Fill(2),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .areas(log_area);

        let paragraph = Paragraph::new(lines).scroll((commits_scroll_idx as u16, 0));
        let block_commits = Block::bordered();
        frame.render_widget(paragraph.block(block_commits), commit_area);

        let paragraph = Paragraph::new(authors).scroll((commits_scroll_idx as u16, 0));
        let block_author = Block::bordered();
        frame.render_widget(paragraph.block(block_author), author_area);

        let paragraph = Paragraph::new(times).scroll((commits_scroll_idx as u16, 0));
        let block_times = Block::bordered();
        frame.render_widget(paragraph.block(block_times), times_area);

        Ok(())
    }
    fn render_commit_area(&mut self, _diff_area: Rect) -> Result<RenderedDiff, std::io::Error> {
        let _ = self
            .get_or_refresh_selected_commit()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let Some(selected_commit) = self.selected_commit_cached.as_ref() else {
            return Ok(RenderedDiff { texts: vec![] });
        };
        Ok(match selected_commit {
            Detail::CommitDetail(selected_commit) => {
                self.render_commit_area_commit(_diff_area, selected_commit)?
            }
            Detail::DiffIndexCommit(diff) | Detail::DiffTreeIndex(diff) => {
                self.render_commit_area_diff(_diff_area, diff)?
            }
            Detail::Error(e) => {
                let diff = &Diff {
                    files: vec![(
                        FileModificationKind::Modification,
                        "ERROR".into(),
                        format!("Error: {e:?}"),
                    )],
                };
                self.render_commit_area_diff(_diff_area, diff)?
            }
        })
    }

    fn render_commit_area_diff(
        &self,
        _diff_area: Rect,
        diff: &Diff,
    ) -> Result<RenderedDiff, std::io::Error> {
        self.render_commit_area_diff_inner(_diff_area, diff, 0, true)
    }

    fn render_commit_area_diff_inner(
        &self,
        _diff_area: Rect,
        diff: &Diff,
        prior_text_len: usize,
        can_set_bold: bool,
    ) -> Result<RenderedDiff, std::io::Error> {
        let mut bold_already_set = !can_set_bold;
        let diff_scroll_idx = self.diff_scroll_idx;
        let mut texts = Vec::new();

        let mut len_ctr = prior_text_len;

        for (kind, path, diff) in diff.files.iter().filter(|(kind, _path, diff)| {
            matches!(kind, crate::model::FileModificationKind::Rewrite(_))
                || !diff.trim().is_empty()
        }) {
            let st = Style::default();
            let (kind_str, style) = match kind {
                crate::model::FileModificationKind::Addition => ('A', st.green()),
                crate::model::FileModificationKind::Deletion => ('D', st.red()),
                crate::model::FileModificationKind::Modification => ('M', st.yellow()),
                crate::model::FileModificationKind::Rewrite(_) => ('R', st.yellow()),
            };
            let mut diff_for_file = Text::from(vec![Line::styled(
                dash_wrap(path),
                Style::default().white().on_dark_gray(),
            )]);
            if let crate::model::FileModificationKind::Rewrite(source_loc) = kind {
                let renamed_line = Line::styled(
                    format!("Renamed from: {source_loc}"),
                    Style::default().white().on_dark_gray(),
                );
                diff_for_file.extend(Text::from(vec![renamed_line]));
            }
            diff_for_file.extend(style_text_for_diff(diff));
            diff_for_file.extend([Line::from("")]);

            let style =
                if len_ctr + diff_for_file.lines.len() > diff_scroll_idx && !bold_already_set {
                    bold_already_set = true;
                    style.bold().on_dark_gray()
                } else {
                    style
                };
            len_ctr += diff_for_file.lines.len();

            let index_line = Line::from(format!("{kind_str} {path}")).style(style);
            texts.push((index_line, diff_for_file));
        }

        Ok(RenderedDiff { texts })
    }
    fn render_commit_area_commit(
        &self,
        _diff_area: Rect,
        selected_commit: &CommitDetail,
    ) -> Result<RenderedDiff, std::io::Error> {
        let diff_scroll_idx = self.diff_scroll_idx;
        let mut texts = Vec::new();

        fn line_with_kind<'a>(kind: &'a str, s: String) -> Line<'a> {
            Line::from(vec![Span::from(kind).bold(), Span::from(s)])
        }
        let parents_str = selected_commit
            .parents
            .iter()
            .map(|(_oid, oid_prefix, ttl)| format!("{oid_prefix} {ttl}"))
            .collect::<Vec<String>>();
        let parents_str = parents_str.join(", ");
        let mut commit_descr_text = Text::from(vec![
            line_with_kind("Author: ", selected_commit.author.format_with_time()),
            line_with_kind("Committer: ", selected_commit.committer.format_with_time()),
            line_with_kind("Parents: ", parents_str),
            Line::from(""),
            Line::from(selected_commit.title.clone()),
            Line::from(""),
        ]);
        commit_descr_text.extend(Text::raw(selected_commit.msg_detail.clone()));
        commit_descr_text.extend([Line::from("")]);

        let mut bold_already_set = false;

        let st = if commit_descr_text.lines.len() > diff_scroll_idx && !bold_already_set {
            bold_already_set = true;
            Style::default().bold().on_dark_gray()
        } else {
            Style::default()
        };

        let descr_text_len = commit_descr_text.lines.len();

        texts.push((Line::from("Description").style(st), commit_descr_text));

        let diff = &selected_commit.diff_parent;

        let texts_diff = self.render_commit_area_diff_inner(
            _diff_area,
            diff,
            descr_text_len,
            !bold_already_set,
        )?;

        texts.extend(texts_diff.texts);

        Ok(RenderedDiff { texts })
    }
    fn draw_selected_commit_area(
        &mut self,
        frame: &mut Frame,
        diff_area: Rect,
    ) -> Result<(), std::io::Error> {
        let diff_scroll_idx = self.diff_scroll_idx;
        let rendered_diff = self.render_commit_area(diff_area)?;
        self.last_rendered_diff = Some(rendered_diff.clone());

        let Some(selected_commit) = self
            .get_or_refresh_selected_commit()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        else {
            return Ok(());
        };
        if rendered_diff.texts.len() == 0 {
            return Ok(());
        }

        let [commit_descr_area, files_area] =
            Layout::horizontal([Constraint::Fill(3), Constraint::Fill(1)]).areas(diff_area);

        let files_lines = rendered_diff
            .texts
            .iter()
            .map(|l| l.0.clone())
            .collect::<Vec<_>>();
        let commit_descr_text =
            rendered_diff
                .texts
                .into_iter()
                .fold(Text::from(Vec::new()), |mut t_a, (_l_b, t_b)| {
                    t_a.extend(t_b);
                    t_a
                });

        let mut scrollbar_state = ScrollbarState::new(commit_descr_text.lines.len());
        scrollbar_state = scrollbar_state.position(diff_scroll_idx);

        let paragraph = Paragraph::new(commit_descr_text)
            .wrap(Wrap { trim: false })
            .scroll((diff_scroll_idx as u16, 0));

        let scrollbar_area = commit_descr_area.inner(ratatui::layout::Margin {
            vertical: 0,
            horizontal: 1,
        });
        frame.render_stateful_widget(
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
            scrollbar_area,
            &mut scrollbar_state,
        );

        let title = match selected_commit {
            Detail::CommitDetail(selected_commit) => format!("Commit {}", selected_commit.id),
            Detail::DiffTreeIndex(_) | Detail::DiffIndexCommit(_) => format!("Diff"),
            Detail::Error(_) => format!("Error"),
        };
        let block_selected = Block::bordered().title(title);
        frame.render_widget(paragraph.block(block_selected), commit_descr_area);

        let paragraph = Paragraph::new(files_lines).wrap(Wrap { trim: true });
        let block_selected = Block::bordered();
        frame.render_widget(paragraph.block(block_selected), files_area);

        Ok(())
    }
    pub(crate) fn commits_authors_times_lines(
        &mut self,
    ) -> Result<(Vec<Line<'_>>, Vec<Line<'_>>, Vec<Line<'_>>), anyhow::Error> {
        // cache the commits to display so that we don't do IO at each render iteration
        let selection_idx = self.selection_idx;
        let commits_shallow = self.get_or_refresh_commits_shallow()?;
        let [mut lines, mut authors, mut times]: [Vec<_>; 3] = Default::default();

        let selected_st = ratatui::style::Modifier::BOLD | ratatui::style::Modifier::UNDERLINED;
        for (idx, cmt) in commits_shallow.iter().enumerate() {
            let commit_id_st = Style::default().yellow();
            let commit_line = match cmt.id {
                crate::model::ShallowId::CommitId(_id, prefix) => Line::from(vec![
                    Span::from(prefix.to_string()).style(commit_id_st),
                    Span::from(format!(" {}", cmt.commit)),
                ]),
                crate::model::ShallowId::Worktree | crate::model::ShallowId::Index => {
                    Line::from(cmt.commit.clone())
                }
            };
            if idx == selection_idx {
                lines.push(commit_line.style(selected_st));
                authors.push(Line::from(cmt.signature.to_string()).style(selected_st));
                times.push(Line::from(cmt.signature.time.clone()).style(selected_st));
            } else {
                lines.push(commit_line);
                authors.push(Line::from(cmt.signature.to_string()));
                times.push(Line::from(cmt.signature.time.clone()));
            }
        }
        Ok((lines, authors, times))
    }
}

/// Wrap the given string in dashes, i.e. `---- abc ----`
fn dash_wrap(s: &str) -> String {
    let pad_to_len = 80usize;
    let padding_len = pad_to_len.saturating_sub(s.len());
    let nothing = "";
    let pad_left = padding_len / 2;
    // Compute number again as padding_len could be odd
    let pad_right = padding_len - pad_left;
    format!("{nothing:->pad_left$} {s} {nothing:->pad_right$}")
}

fn style_text_for_diff(diff: &str) -> Text<'static> {
    let lines = diff
        .lines()
        .map(|line| {
            let st = if line.starts_with('+') {
                Style::default().green()
            } else if line.starts_with('-') {
                Style::default().red()
            } else if line.starts_with("@@") {
                Style::default().blue()
            } else {
                Style::default()
            };
            Line::from(line.to_owned()).style(st)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}
