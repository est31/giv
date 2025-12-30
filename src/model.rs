use gix::{ObjectId, actor::SignatureRef, diff::blob::{UnifiedDiff, unified_diff::{ConsumeBinaryHunk, ContextSize}}, hash::Prefix};

use crate::State;

pub(crate) struct CommitShallow {
    pub(crate) id: ObjectId,
    pub(crate) commit: String,
    pub(crate) signature: Signature,
}

pub(crate) struct Signature {
    pub(crate) author_name: String,
    pub(crate) author_email: String,
    pub(crate) time: String,
}

pub(crate) struct CommitDetail {
    pub(crate) author: Signature,
    pub(crate) committer: Signature,
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
    pub(crate) files: Vec<(FileModificationKind, String, String)>,
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <{}>", self.author_name, self.author_email)
    }
}

impl Signature {
    pub(crate) fn format_with_time(&self) -> String {
        format!("{} <{}> {}", self.author_name, self.author_email, self.time)
    }
}

impl State {
    fn make_signature(&self, sig: SignatureRef<'_>) -> Result<Signature, anyhow::Error> {
        Ok(Signature {
            author_name: sig.name.to_string().trim().to_owned(),
            author_email: sig.email.to_string().trim().to_owned(),
            time: sig.time()?.format(gix::date::time::format::ISO8601)?,
        })
    }
    pub(crate) fn get_or_refresh_commits_shallow(&mut self) -> Result<&[CommitShallow], anyhow::Error> {
        if self.commits_shallow_cached.is_none() {
            let head_commit = self.repo.head_commit()?;
            let msg = head_commit.message()?;
            let id = head_commit.id().shorten_or_id();
            let title = msg.title.to_string();
            let mut res = Vec::new();
            res.push(CommitShallow {
                id: head_commit.id,
                commit: format!("{} {}", id, title.trim()),
                signature: self.make_signature(head_commit.author()?)?,
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
                    signature: self.make_signature(commit.author()?)?,
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
        let author = self.make_signature(commit.author()?)?;
        let committer = self.make_signature(commit.committer()?)?;
        let parents = commit.parent_ids()
            .map(|id| {
                let parent_commit = self.repo.find_commit(id)?;
                let msg = parent_commit.message()?.title.to_string().trim().to_owned();
                Ok((id.into(), id.shorten_or_id(), msg))
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;
        let diff_parent = self.compute_diff(commit)?;
        Ok(Some(CommitDetail { author, committer, parents, title, msg_detail, diff_parent }))
    }
    fn compute_diff(&self, commit: gix::Commit<'_>) -> Result<Diff, anyhow::Error> {
        let Some(parent_id) = commit.parent_ids().next() else {
            return Ok(Diff { files: Vec::new() });
        };
        let parent = self.repo.find_commit(parent_id)?;
        let diff_options = None;
        let diff_changes = self.repo.diff_tree_to_tree(&parent.tree()?, &commit.tree()?, diff_options)?;
        let mut files = diff_changes.iter().map(|chg| {
            let chg = match chg {
                gix::diff::tree_with_rewrites::Change::Addition { location, id, .. } => {
                    let location_str = location.to_string().trim().to_owned();
                    let diff_text = if self.repo.find_object(*id)?.kind == gix::objs::Kind::Blob {
                        let now_blob = self.repo.find_blob(*id)?;

                        let interner = gix::diff::blob::intern::InternedInput::new(&b""[..], now_blob.data.as_slice());

                        let diff_str_raw = gix::diff::blob::diff(
                            gix::diff::blob::Algorithm::Myers,
                            &interner,
                            UnifiedDiff::new(
                                &interner,
                                ConsumeBinaryHunk::new(String::new(), "\n"),
                                ContextSize::symmetrical(3),
                            ),
                        )?;
                        format!("Changes for {location_str}\n{diff_str_raw}")
                    } else {
                        String::new()
                    };
                    (FileModificationKind::Addition, location_str, diff_text)
                },
                gix::diff::tree_with_rewrites::Change::Deletion { location, .. } => {
                    (FileModificationKind::Deletion, location.to_string().trim().to_owned(), String::new())
                },
                gix::diff::tree_with_rewrites::Change::Modification { location, previous_id, id, .. } => {
                    let location_str = location.to_string().trim().to_owned();
                    let diff_text = if self.repo.find_object(*id)?.kind == gix::objs::Kind::Blob {
                        let prev_blob = self.repo.find_blob(*previous_id)?;
                        let now_blob = self.repo.find_blob(*id)?;

                        let interner = gix::diff::blob::intern::InternedInput::new(prev_blob.data.as_slice(), now_blob.data.as_slice());

                        let diff_str_raw = gix::diff::blob::diff(
                            gix::diff::blob::Algorithm::Myers,
                            &interner,
                            UnifiedDiff::new(
                                &interner,
                                ConsumeBinaryHunk::new(String::new(), "\n"),
                                ContextSize::symmetrical(3),
                            ),
                        )?;
                        format!("Changes for {location_str}\n{diff_str_raw}")
                    } else {
                        String::new()
                    };
                    (FileModificationKind::Modification, location_str, diff_text)
                },
                gix::diff::tree_with_rewrites::Change::Rewrite { location, .. } => {
                    (FileModificationKind::Rewrite, location.to_string().trim().to_owned(), String::new())
                },
            };
            Ok(chg)
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;
        files.sort_by_cached_key(|f| f.1.clone());
        Ok(Diff { files })
    }
    pub(crate) fn invalidate_caches(&mut self) {
        self.commits_shallow_cached = None;
    }
}