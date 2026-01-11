use std::collections::BTreeSet;

use gix::{
    ObjectId,
    actor::SignatureRef,
    diff::blob::{
        UnifiedDiff,
        unified_diff::{ConsumeBinaryHunk, ContextSize},
    },
    hash::Prefix,
    hashtable::hash_set::HashSet,
};

use crate::State;

pub(crate) struct CommitShallow {
    pub(crate) id: ShallowId,
    pub(crate) commit: String,
    pub(crate) signature: Signature,
}

#[derive(Clone, Copy)]
pub(crate) enum ShallowId {
    CommitId(ObjectId, Prefix),
    Worktree,
    Index,
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
    pub(crate) id: ObjectId,
}

#[allow(dead_code)]
pub(crate) enum Detail {
    DiffTreeIndex(Diff),
    DiffIndexCommit(Diff),
    CommitDetail(CommitDetail),
    Error(anyhow::Error),
}

pub(crate) enum FileModificationKind {
    Addition,
    Deletion,
    Modification,
    Rewrite(String),
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
    pub(crate) fn get_or_refresh_commits_shallow(
        &mut self,
    ) -> Result<&[CommitShallow], anyhow::Error> {
        if self.commits_shallow_cached.is_none() {
            let mut res = Vec::new();

            let (worktree_changes, index_changes) = self.has_worktree_index_changes()?;

            if worktree_changes {
                res.push(CommitShallow {
                    id: ShallowId::Worktree,
                    commit: format!("Worktree changes, not in index"),
                    signature: Signature {
                        author_name: String::new(),
                        author_email: String::new(),
                        time: String::new(),
                    },
                });
            }
            if index_changes {
                res.push(CommitShallow {
                    id: ShallowId::Index,
                    commit: format!("Index changes, not in a commit"),
                    signature: Signature {
                        author_name: String::new(),
                        author_email: String::new(),
                        time: String::new(),
                    },
                });
            }

            let head_commit = self.repo.head_commit()?;

            let budget = self.wanted_commit_list_count;

            let mut seen = HashSet::new();
            let mut to_handle = BTreeSet::new();

            to_handle.insert(((), head_commit.id));

            while let Some((_commit_date, commit_id)) = to_handle.pop_first() {
                if res.len() > budget {
                    break;
                }
                let commit = self.repo.find_commit(commit_id)?;
                for parent_id in commit.parent_ids() {
                    if !seen.insert(parent_id.detach()) {
                        continue;
                    }
                    to_handle.insert(((), parent_id.detach()));
                }
                let msg = commit.message()?;
                let title = msg.title.to_string();
                res.push(CommitShallow {
                    id: ShallowId::CommitId(commit.id, commit.short_id()?),
                    commit: format!("{}", title.trim()),
                    signature: self.make_signature(commit.author()?)?,
                });
            }
            Ok(self.commits_shallow_cached.insert(res))
        } else {
            Ok(self.commits_shallow_cached.as_ref().unwrap())
        }
    }
    pub(crate) fn has_worktree_index_changes(&mut self) -> Result<(bool, bool), anyhow::Error> {
        if let Some(cached) = self.worktree_index_changed_cached {
            return Ok(cached);
        }

        let iter = self
            .repo
            .status(gix::progress::Discard)?
            .index_worktree_rewrites(None)
            .index_worktree_submodules(gix::status::Submodule::AsConfigured { check_dirty: true })
            .untracked_files(gix::status::UntrackedFiles::Collapsed)
            .index_worktree_options_mut(|opts| {
                opts.dirwalk_options = None;
            })
            .into_iter(Vec::new())?;

        let mut worktree_changes = false;
        let mut index_changes = false;

        for it in iter {
            match it? {
                gix::status::Item::IndexWorktree(_) => worktree_changes = true,
                gix::status::Item::TreeIndex(_) => index_changes = true,
            }
        }
        let res = (worktree_changes, index_changes);
        self.worktree_index_changed_cached = Some(res);

        Ok(res)
    }
    pub(crate) fn get_or_refresh_selected_commit(
        &mut self,
    ) -> Result<Option<&Detail>, anyhow::Error> {
        if self.selected_commit_cached.is_none() {
            let selected_opt_res = self.get_selected_commit();
            let selected_opt = match selected_opt_res {
                Ok(v) => v,
                Err(e) => Some(Detail::Error(e)),
            };
            if let Some(selected) = selected_opt {
                Ok(Some(self.selected_commit_cached.insert(selected)))
            } else {
                Ok(None)
            }
        } else {
            Ok(self.selected_commit_cached.as_ref())
        }
    }
    fn get_selected_commit(&mut self) -> Result<Option<Detail>, anyhow::Error> {
        let selection_idx = self.selection_idx;
        let index_id = {
            let selected_hash = self.get_or_refresh_commits_shallow()?;
            let Some(selected_commit) = selected_hash.get(selection_idx) else {
                return Ok(None);
            };
            selected_commit.id
        };
        let id = match index_id {
            ShallowId::CommitId(id, _prefix) => id,
            ShallowId::Worktree => {
                return Ok(Some(Detail::DiffTreeIndex(
                    self.compute_diff_worktree_to_index()?,
                )));
            }
            ShallowId::Index => {
                return Ok(Some(Detail::DiffIndexCommit(
                    self.compute_diff_index_to_commit()?,
                )));
            }
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
        let parents = commit
            .parent_ids()
            .map(|id| {
                let parent_commit = self.repo.find_commit(id)?;
                let msg = parent_commit.message()?.title.to_string().trim().to_owned();
                Ok((id.into(), id.shorten_or_id(), msg))
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;
        let diff_parent = match self.compute_diff_commit(commit) {
            Ok(d) => d,
            // TODO this is a bit of a hack, but it allows us to separate error domains
            Err(e) => Diff {
                files: vec![(
                    FileModificationKind::Deletion,
                    "ERROR".to_owned(),
                    format!("error: {e}"),
                )],
            },
        };
        let commit_detail = CommitDetail {
            author,
            committer,
            parents,
            title,
            msg_detail,
            diff_parent,
            id,
        };
        Ok(Some(Detail::CommitDetail(commit_detail)))
    }
    fn compute_diff_worktree_to_index(&self) -> Result<Diff, anyhow::Error> {
        let iter = self
            .repo
            .status(gix::progress::Discard)?
            .index_worktree_rewrites(None)
            .index_worktree_submodules(gix::status::Submodule::AsConfigured { check_dirty: true })
            .index_worktree_options_mut(|opts| {
                opts.dirwalk_options = None;
            })
            .into_index_worktree_iter(Vec::new())?;
        let files = iter
            .map(|v| match v {
                Ok(gix::status::index_worktree::Item::Modification {
                    entry, rela_path, ..
                }) => {
                    // TODO don't use unwrap here but return dedicated ERR item
                    let worktree = self.repo.worktree().unwrap();
                    let in_worktree =
                        std::fs::read(worktree.base().join(rela_path.to_string())).unwrap();

                    let obj = self.repo.find_object(entry.id).unwrap();

                    let interner = gix::diff::blob::intern::InternedInput::new(
                        obj.data.as_slice(),
                        in_worktree.as_slice(),
                    );
                    let diff_str_raw = gix::diff::blob::diff(
                        gix::diff::blob::Algorithm::Myers,
                        &interner,
                        UnifiedDiff::new(
                            &interner,
                            ConsumeBinaryHunk::new(String::new(), "\n"),
                            ContextSize::symmetrical(3),
                        ),
                    )
                    .unwrap();
                    let diff_str_raw = format!("{diff_str_raw}\nworktree to {}", entry.id);
                    (
                        FileModificationKind::Modification,
                        format!("{}", rela_path),
                        diff_str_raw,
                    )
                }
                Ok(gix::status::index_worktree::Item::DirectoryContents { entry, .. }) => (
                    FileModificationKind::Addition,
                    format!("{}", entry.rela_path),
                    "New file".to_owned(),
                ),
                Ok(gix::status::index_worktree::Item::Rewrite { dirwalk_entry, .. }) => (
                    FileModificationKind::Modification,
                    format!("{}", dirwalk_entry.rela_path),
                    "...".to_owned(),
                ),
                Err(e) => (
                    FileModificationKind::Modification,
                    format!("ERR"),
                    format!("error: {e}"),
                ),
            })
            .collect();
        return Ok(Diff { files });
    }
    fn compute_diff_index_to_commit(&self) -> Result<Diff, anyhow::Error> {
        let iter = self
            .repo
            .status(gix::progress::Discard)?
            .index_worktree_rewrites(None)
            .index_worktree_submodules(gix::status::Submodule::AsConfigured { check_dirty: true })
            .untracked_files(gix::status::UntrackedFiles::Collapsed)
            .index_worktree_options_mut(|opts| {
                opts.dirwalk_options = None;
            })
            .into_iter(Vec::new())?;

        let mut files = Vec::new();

        for v in iter {
            let gix::status::Item::TreeIndex(change) = v? else {
                continue;
            };
            let file = match change {
                gix::diff::index::ChangeRef::Addition {
                    location, id: _, ..
                } => (
                    FileModificationKind::Addition,
                    format!("{location}"),
                    "...".to_owned(),
                ),
                gix::diff::index::ChangeRef::Deletion {
                    location, id: _, ..
                } => (
                    FileModificationKind::Deletion,
                    format!("{location}"),
                    "...".to_owned(),
                ),

                gix::diff::index::ChangeRef::Rewrite {
                    location,
                    source_id: previous_id,
                    id,
                    ..
                }
                | gix::diff::index::ChangeRef::Modification {
                    location,
                    previous_id,
                    id,
                    ..
                } => {
                    // TODO don't use unwrap here but return dedicated ERR item
                    let prev_obj = self.repo.find_object(&*previous_id.to_owned()).unwrap();
                    let now_obj = self.repo.find_object(&*id.to_owned()).unwrap();

                    let interner = gix::diff::blob::intern::InternedInput::new(
                        prev_obj.data.as_slice(),
                        now_obj.data.as_slice(),
                    );
                    let diff_str_raw = gix::diff::blob::diff(
                        gix::diff::blob::Algorithm::Myers,
                        &interner,
                        UnifiedDiff::new(
                            &interner,
                            ConsumeBinaryHunk::new(String::new(), "\n"),
                            ContextSize::symmetrical(3),
                        ),
                    )
                    .unwrap();
                    (
                        FileModificationKind::Modification,
                        format!("{location}"),
                        diff_str_raw,
                    )
                }
            };
            files.push(file);
        }
        return Ok(Diff { files });
    }
    fn compute_diff_commit(&self, commit: gix::Commit<'_>) -> Result<Diff, anyhow::Error> {
        let Some(parent_id) = commit.parent_ids().next() else {
            return Ok(Diff { files: Vec::new() });
        };
        let parent = self.repo.find_commit(parent_id)?;
        let diff_options = None;
        let diff_changes =
            self.repo
                .diff_tree_to_tree(&parent.tree()?, &commit.tree()?, diff_options)?;
        let mut files = diff_changes
            .iter()
            .map(|chg| {
                let (kind, location_str, prev_id_opt, now_id_opt) = match chg {
                    gix::diff::tree_with_rewrites::Change::Addition { location, id, .. } => {
                        let location_str = location.to_string().trim().to_owned();
                        (
                            FileModificationKind::Addition,
                            location_str,
                            None,
                            Some(*id),
                        )
                    }
                    gix::diff::tree_with_rewrites::Change::Deletion { location, .. } => (
                        FileModificationKind::Deletion,
                        location.to_string().trim().to_owned(),
                        None,
                        None,
                    ),
                    gix::diff::tree_with_rewrites::Change::Modification {
                        location,
                        previous_id,
                        id,
                        ..
                    } => {
                        let location_str = location.to_string().trim().to_owned();
                        (
                            FileModificationKind::Modification,
                            location_str,
                            Some(*previous_id),
                            Some(*id),
                        )
                    }
                    gix::diff::tree_with_rewrites::Change::Rewrite {
                        source_location,
                        location,
                        source_id,
                        id,
                        ..
                    } => {
                        let source_location_str = source_location.to_string().trim().to_owned();
                        let location_str = location.to_string().trim().to_owned();
                        (
                            FileModificationKind::Rewrite(source_location_str),
                            location_str,
                            Some(*source_id),
                            Some(*id),
                        )
                    }
                };
                let diff_text = if let Some(id) = now_id_opt
                    && self.repo.find_object(id)?.kind == gix::objs::Kind::Blob
                {
                    let now_blob = self.repo.find_blob(id)?;
                    let mut prev_blob = None;
                    let interner = if let Some(prev_id) = prev_id_opt {
                        let prev_blob_ref = prev_blob.insert(self.repo.find_blob(prev_id)?);

                        gix::diff::blob::intern::InternedInput::new(
                            prev_blob_ref.data.as_slice(),
                            now_blob.data.as_slice(),
                        )
                    } else {
                        gix::diff::blob::intern::InternedInput::new(
                            b"".as_slice(),
                            now_blob.data.as_slice(),
                        )
                    };

                    let diff_str_raw = gix::diff::blob::diff(
                        gix::diff::blob::Algorithm::Myers,
                        &interner,
                        UnifiedDiff::new(
                            &interner,
                            ConsumeBinaryHunk::new(String::new(), "\n"),
                            ContextSize::symmetrical(3),
                        ),
                    )?;
                    diff_str_raw
                } else {
                    String::new()
                };

                Ok((kind, location_str, diff_text))
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;
        files.sort_by_cached_key(|f| f.1.clone());
        Ok(Diff { files })
    }
    pub(crate) fn invalidate_caches(&mut self) {
        self.commits_shallow_cached = None;
        self.selected_commit_cached = None;
    }
}
