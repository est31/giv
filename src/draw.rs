use ratatui::{
    Frame, layout::{Constraint, Layout}, style::Stylize, text::{Line, Span, Text}, widgets::{Block, Paragraph, Wrap}
};

use super::State;

impl State {
    pub(crate) fn draw(&mut self, frame: &mut Frame) -> Result<(), std::io::Error> {
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
            let [commit_descr_area, files_area] = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(1)]).areas(diff_area);
            fn line_with_kind<'a>(kind: &'a str, s: String) -> Line<'a> {
                Line::from(vec![Span::from(kind).bold(), Span::from(s)])
            }
            let parents_str = selected_commit.parents.iter().map(|(_oid, oid_prefix, ttl)| format!("{oid_prefix} {ttl}"))
                .collect::<Vec<String>>();
            let parents_str = parents_str.join(", ");
            let mut text = Text::from(vec![
                line_with_kind("Author: ", selected_commit.author.format_with_time()),
                line_with_kind("Committer: ", selected_commit.committer.format_with_time()),
                line_with_kind("Parents: ", parents_str),
                Line::from(""),
                Line::from(selected_commit.title),
                Line::from(""),
            ]);
            text.extend(Text::raw(selected_commit.msg_detail));

            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: true });
            let block_selected = Block::bordered();
            frame.render_widget(paragraph.block(block_selected), commit_descr_area);

            let files_lines = selected_commit.diff_parent.files.iter()
                .map(|(kind, path)| {
                    let kind_str = match kind {
                        crate::model::FileModificationKind::Addition => 'A',
                        crate::model::FileModificationKind::Deletion => 'D',
                        crate::model::FileModificationKind::Modification => 'M',
                        crate::model::FileModificationKind::Rewrite => 'R',
                    };
                    Line::from(format!("{kind_str} {path}"))
                })
                .collect::<Vec<_>>();

            let paragraph = Paragraph::new(files_lines)
                .wrap(Wrap { trim: true });
            let block_selected = Block::bordered();
            frame.render_widget(paragraph.block(block_selected), files_area);
        }
        Ok(())
    }
    pub(crate) fn commits_authors_times_lines(&mut self) -> Result<(Vec<Line<'_>>, Vec<Line<'_>>, Vec<Line<'_>>), anyhow::Error> {
        // cache the commits to display so that we don't do IO at each render iteration
        let selection_idx = self.selection_idx;
        let commits_shallow = self.get_or_refresh_commits_shallow()?;
        let [mut lines, mut authors, mut times]: [Vec<_>; 3] = Default::default();

        let selected_st = ratatui::style::Modifier::BOLD;
        for (idx, cmt) in commits_shallow.iter().enumerate() {
            if Some(idx) == selection_idx {
                lines.push(Line::from(cmt.commit.clone()).style(selected_st));
                authors.push(Line::from(cmt.signature.to_string()).style(selected_st));
                times.push(Line::from(cmt.signature.time.clone()).style(selected_st));
            } else {
            lines.push(Line::from(cmt.commit.clone()));
            authors.push(Line::from(cmt.signature.to_string()));
            times.push(Line::from(cmt.signature.time.clone()));
            }
        }
        Ok((lines, authors, times))
    }
}