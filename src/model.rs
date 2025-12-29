use gix::{ObjectId, hash::Prefix};
use ratatui::{text::Line,};

use crate::State;

pub(crate) struct CommitShallow {
    pub(crate) id: ObjectId,
    pub(crate) commit: String,
    pub(crate) author: String,
    pub(crate) time: String,
}

pub(crate) struct CommitDetail {
    pub(crate) commit: String,
    pub(crate) author: String,
    pub(crate) committer: String,
    pub(crate) title: String,
    pub(crate) msg_detail: String,
    pub(crate) parents: Vec<(ObjectId, Prefix, String)>,
    pub(crate) diff_parent: Diff,
}

pub(crate) enum FileModificationKind {
    Addition,
    Deletion,
    Modification,
    Rewrite,
}

pub(crate) struct Diff {
    pub(crate) files: Vec<(FileModificationKind, String)>,
}

impl State {
    pub(crate) fn commits_authors_times_lines(&mut self) -> Result<(Vec<Line<'_>>, Vec<Line<'_>>, Vec<Line<'_>>), anyhow::Error> {
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
    pub(crate) fn get_or_refresh_commits_shallow(&mut self) -> Result<&[CommitShallow], anyhow::Error> {
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
    pub(crate) fn get_selected_commit(&mut self) -> Result<Option<CommitDetail>, anyhow::Error> {
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
        let parents = commit.parent_ids()
            .map(|id| {
                let parent_commit = self.repo.find_commit(id)?;
                let msg = parent_commit.message()?.title.to_string().trim().to_owned();
                Ok((id.into(), id.shorten_or_id(), msg))
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;
        let diff_parent = self.compute_diff(commit)?;
        let commit = String::new();
        Ok(Some(CommitDetail { commit, author, committer, parents, title, msg_detail, diff_parent }))
    }
    fn compute_diff(&self, commit: gix::Commit<'_>) -> Result<Diff, anyhow::Error> {
        let Some(parent_id) = commit.parent_ids().next() else {
            return Ok(Diff { files: Vec::new() });
        };
        let parent = self.repo.find_commit(parent_id)?;
        let diff_options = None;
        let diff_changes = self.repo.diff_tree_to_tree(&commit.tree()?, &parent.tree()?, diff_options)?;
        let files = diff_changes.iter().map(|chg| {
            let chg = match chg {
                gix::diff::tree_with_rewrites::Change::Addition { location, .. } => {
                    (FileModificationKind::Addition, location.to_string().trim().to_owned())
                },
                gix::diff::tree_with_rewrites::Change::Deletion { location, .. } => {
                    (FileModificationKind::Deletion, location.to_string().trim().to_owned())
                },
                gix::diff::tree_with_rewrites::Change::Modification { location, .. } => {
                    (FileModificationKind::Modification, location.to_string().trim().to_owned())
                },
                gix::diff::tree_with_rewrites::Change::Rewrite { location, .. } => {
                    (FileModificationKind::Rewrite, location.to_string().trim().to_owned())
                },
            };
            Ok(chg)
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;
        Ok(Diff { files })
    }
    pub(crate) fn invalidate_caches(&mut self) {
        self.commits_shallow_cached = None;
    }
}