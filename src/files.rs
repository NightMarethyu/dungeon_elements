//! Undo and Redo for changes to the campaign's files and folders: new notes and folders,
//! renames, moves, deletes, and saving Note Types or walkthrough answers.
//!
//! The campaign's text files (notes and `_type.yaml`s) are kept in memory as they were
//! last seen, with the size and change time of every other file. After the app changes
//! something, [`Snapshot::changes`] looks for files whose size or time changed, reads
//! only those, and returns what changed. A [`Step`] holds just those files, before and
//! after, so undoing a rename that touched 40 notes keeps those 40 notes and nothing else.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

/// At most this many campaign steps are kept; the oldest go first.
pub const MAX_STEPS: usize = 200;
/// At most this much file content is kept in the steps, in bytes; the oldest go first.
pub const MAX_BYTES: usize = 50 * 1024 * 1024;
/// Files other than notes (pictures and the like) bigger than this, together in one
/// step, aren't copied; undoing their delete leaves them in the Recycle Bin.
pub const MAX_OTHER_BYTES: u64 = 20 * 1024 * 1024;

/// What a path held: a folder, or a file with its bytes (`None` when too big to keep).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Folder,
    File(Option<Arc<[u8]>>),
}

impl Content {
    fn size(&self) -> usize {
        match self {
            Content::File(Some(bytes)) => bytes.len(),
            _ => 0,
        }
    }
}

/// One path that changed: what it held before and after (`None`: nothing there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub before: Option<Content>,
    pub after: Option<Content>,
}

/// Whether the app keeps a copy of the file all the time: notes and YAML files.
pub fn is_text(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "yaml" | "yml" | "txt"
            )
        })
}

#[derive(Debug, Clone)]
struct Seen {
    len: u64,
    modified: Option<SystemTime>,
    /// The bytes, for text files, and for other files while a delete is under way.
    bytes: Option<Arc<[u8]>>,
}

/// The campaign's files as the app last saw them.
#[derive(Debug, Default)]
pub struct Snapshot {
    files: HashMap<PathBuf, Seen>,
    folders: BTreeSet<PathBuf>,
}

impl Snapshot {
    /// Reads the campaign folder at `root`: every folder, the text of every note and
    /// YAML file, and the size and time of everything else.
    pub fn take(root: &Path) -> Self {
        let mut snapshot = Snapshot::default();
        let mut listed = Listing::default();
        list(root, &mut listed);
        for (path, (len, modified)) in listed.files {
            let bytes = is_text(&path)
                .then(|| std::fs::read(&path).ok().map(Arc::from))
                .flatten();
            snapshot.files.insert(
                path,
                Seen {
                    len,
                    modified,
                    bytes,
                },
            );
        }
        snapshot.folders = listed.folders;
        snapshot
    }

    /// Keeps a copy of the files under `path` that aren't text (pictures and the like),
    /// up to [`MAX_OTHER_BYTES`] together, so deleting them can be undone.
    pub fn keep_files_under(&mut self, path: &Path) {
        let mut total = 0;
        for (file, seen) in &mut self.files {
            if seen.bytes.is_some() || !file.starts_with(path) {
                continue;
            }
            total += seen.len;
            if total > MAX_OTHER_BYTES {
                return;
            }
            seen.bytes = std::fs::read(file).ok().map(Arc::from);
        }
    }

    /// What changed in the campaign folder at `root` since the snapshot was taken or
    /// last looked at, which it now holds. Only files whose size or time changed are read.
    pub fn changes(&mut self, root: &Path) -> Vec<Change> {
        let mut listed = Listing::default();
        list(root, &mut listed);
        let mut changes = Vec::new();

        for folder in listed.folders.difference(&self.folders) {
            changes.push(Change {
                path: folder.clone(),
                before: None,
                after: Some(Content::Folder),
            });
        }
        for folder in self.folders.difference(&listed.folders) {
            changes.push(Change {
                path: folder.clone(),
                before: Some(Content::Folder),
                after: None,
            });
        }

        let mut files = HashMap::with_capacity(listed.files.len());
        for (path, (len, modified)) in listed.files {
            let old = self.files.remove(&path);
            let unchanged = old
                .as_ref()
                .is_some_and(|old| old.len == len && old.modified == modified);
            if unchanged && let Some(old) = old {
                files.insert(path, old);
                continue;
            }
            let read = || std::fs::read(&path).ok().map(Arc::<[u8]>::from);
            let bytes = if is_text(&path) || len <= MAX_OTHER_BYTES {
                read()
            } else {
                None
            };
            let before = old.as_ref().map(|old| Content::File(old.bytes.clone()));
            // A file saved again as it was isn't a change.
            let same = match (&old, &bytes) {
                (Some(old), Some(new)) => old.bytes.as_deref() == Some(&**new),
                _ => false,
            };
            if !same {
                changes.push(Change {
                    path: path.clone(),
                    before,
                    after: Some(Content::File(bytes.clone())),
                });
            }
            // Only text is kept between steps; other files' bytes live in the step.
            let kept = bytes.filter(|_| is_text(&path));
            files.insert(
                path,
                Seen {
                    len,
                    modified,
                    bytes: kept,
                },
            );
        }
        for (path, old) in self.files.drain() {
            changes.push(Change {
                path,
                before: Some(Content::File(old.bytes)),
                after: None,
            });
        }
        self.files = files;
        self.folders = listed.folders;
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes
    }
}

#[derive(Default)]
struct Listing {
    files: HashMap<PathBuf, (u64, Option<SystemTime>)>,
    folders: BTreeSet<PathBuf>,
}

/// Lists the files and folders under `folder`, leaving out hidden ones like
/// `.dungeon_elements` and `.obsidian`, which hold settings rather than the campaign.
fn list(folder: &Path, listed: &mut Listing) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            listed.folders.insert(path.clone());
            list(&path, listed);
        } else if kind.is_file()
            && let Ok(metadata) = entry.metadata()
        {
            listed
                .files
                .insert(path, (metadata.len(), metadata.modified().ok()));
        }
    }
}

/// One change to the campaign, like "Move Gorbash".
#[derive(Debug, Clone)]
pub struct Step {
    pub id: u64,
    pub label: String,
    pub changes: Vec<Change>,
    /// Open tabs whose file moved, from where to where, so they follow on Undo.
    pub moves: Vec<(PathBuf, PathBuf)>,
    /// When it was made, or for a step taken back, when it was taken back.
    pub when: Instant,
}

impl Step {
    fn size(&self) -> usize {
        self.changes
            .iter()
            .map(|change| {
                change.before.as_ref().map_or(0, Content::size)
                    + change.after.as_ref().map_or(0, Content::size)
            })
            .sum()
    }
}

/// The campaign's Undo and Redo.
#[derive(Debug, Default)]
pub struct History {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    bytes: usize,
    next_id: u64,
}

impl History {
    /// Adds a step called `label` for `changes`, ending what could be redone. Returns its
    /// id, or `None` when nothing changed.
    pub fn push(
        &mut self,
        label: String,
        changes: Vec<Change>,
        moves: Vec<(PathBuf, PathBuf)>,
    ) -> Option<u64> {
        if changes.is_empty() {
            return None;
        }
        self.next_id += 1;
        let step = Step {
            id: self.next_id,
            label,
            changes,
            moves,
            when: Instant::now(),
        };
        self.bytes += step.size();
        self.undo.push_back(step);
        for step in self.redo.drain(..) {
            self.bytes -= step.size();
        }
        while self.undo.len() > MAX_STEPS || (self.bytes > MAX_BYTES && self.undo.len() > 1) {
            if let Some(step) = self.undo.pop_front() {
                self.bytes -= step.size();
            }
        }
        Some(self.next_id)
    }

    /// The step Undo would take back.
    pub fn last_undo(&self) -> Option<&Step> {
        self.undo.back()
    }

    /// The step Redo would do again.
    pub fn last_redo(&self) -> Option<&Step> {
        self.redo.last()
    }

    /// Takes the last step off the Undo list and puts it on the Redo list, returning it.
    /// The files are changed with [`apply`].
    pub fn undo(&mut self) -> Option<&Step> {
        let mut step = self.undo.pop_back()?;
        step.when = Instant::now();
        self.redo.push(step);
        self.redo.last()
    }

    /// Takes the last step off the Redo list and puts it back on the Undo list.
    pub fn redo(&mut self) -> Option<&Step> {
        let mut step = self.redo.pop()?;
        step.when = Instant::now();
        self.undo.push_back(step);
        self.undo.back()
    }
}

/// What a step's changes go from and to: from after to before for Undo, and the other
/// way for Redo.
fn ends(change: &Change, undo: bool) -> (&Option<Content>, &Option<Content>) {
    if undo {
        (&change.after, &change.before)
    } else {
        (&change.before, &change.after)
    }
}

/// The files that were changed since the step, so undoing (or with `undo` false,
/// redoing) it would lose those changes: changed in another app, or by the app in a
/// way it doesn't undo.
pub fn changed_since(step: &Step, undo: bool) -> Vec<PathBuf> {
    step.changes
        .iter()
        .filter(|change| {
            let (from, _) = ends(change, undo);
            let path = &change.path;
            match from {
                None => path.exists(),
                Some(Content::Folder) => !path.is_dir(),
                Some(Content::File(Some(bytes))) => {
                    std::fs::read(path).ok().as_deref() != Some(&**bytes)
                }
                Some(Content::File(None)) => !path.is_file(),
            }
        })
        .map(|change| change.path.clone())
        .collect()
}

/// Changes the files for Undo (or with `undo` false, for Redo): folders are made first,
/// then files are written, then files and folders that go are removed, deepest first.
/// Returns the files that couldn't be put back, like a big picture that wasn't kept.
pub fn apply(step: &Step, undo: bool) -> Vec<String> {
    let mut problems = Vec::new();
    let mut note = |path: &Path, why: &str| problems.push(format!("{} ({why})", path.display()));

    for change in &step.changes {
        if let (_, Some(Content::Folder)) = ends(change, undo)
            && let Err(error) = std::fs::create_dir_all(&change.path)
        {
            note(&change.path, &error.to_string());
        }
    }
    for change in &step.changes {
        match ends(change, undo) {
            (_, Some(Content::File(Some(bytes)))) => {
                if let Some(parent) = change.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(error) = std::fs::write(&change.path, bytes) {
                    note(&change.path, &error.to_string());
                }
            }
            (_, Some(Content::File(None))) if !change.path.exists() => {
                note(&change.path, "too big to keep a copy of");
            }
            _ => {}
        }
    }
    for change in &step.changes {
        if let (Some(Content::File(_)), None) = ends(change, undo)
            && change.path.is_file()
            && let Err(error) = std::fs::remove_file(&change.path)
        {
            note(&change.path, &error.to_string());
        }
    }
    // Deepest first; a folder something else was put in since stays.
    for change in step.changes.iter().rev() {
        if let (Some(Content::Folder), None) = ends(change, undo) {
            let _ = std::fs::remove_dir(&change.path);
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dungeon_elements_files_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("npcs")).unwrap();
        std::fs::write(root.join("npcs").join("Gorbash.md"), "[[Saltmarsh]]").unwrap();
        std::fs::write(root.join("Saltmarsh.md"), "A town").unwrap();
        root
    }

    fn read(path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    #[test]
    fn a_rename_with_its_links_undoes_and_redoes() {
        let root = folder("rename");
        let mut snapshot = Snapshot::take(&root);
        let mut history = History::default();

        // Rename Saltmarsh, and the link to it.
        std::fs::rename(root.join("Saltmarsh.md"), root.join("Salt Marsh.md")).unwrap();
        std::fs::write(root.join("npcs").join("Gorbash.md"), "[[Salt Marsh]]").unwrap();
        let changes = snapshot.changes(&root);
        assert_eq!(changes.len(), 3, "{changes:#?}");
        history.push("Rename Saltmarsh".into(), changes, Vec::new());
        assert!(snapshot.changes(&root).is_empty());

        let step = history.undo().unwrap().clone();
        assert!(changed_since(&step, true).is_empty());
        assert!(apply(&step, true).is_empty());
        assert_eq!(read(&root.join("Saltmarsh.md")).as_deref(), Some("A town"));
        assert!(!root.join("Salt Marsh.md").exists());
        assert_eq!(
            read(&root.join("npcs").join("Gorbash.md")).as_deref(),
            Some("[[Saltmarsh]]")
        );
        snapshot.changes(&root);

        let step = history.redo().unwrap().clone();
        assert!(apply(&step, false).is_empty());
        assert_eq!(read(&root.join("Salt Marsh.md")).as_deref(), Some("A town"));
        assert!(!root.join("Saltmarsh.md").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deleted_folders_come_back_with_their_pictures() {
        let root = folder("delete");
        let pictures = root.join("npcs").join("pictures");
        std::fs::create_dir_all(&pictures).unwrap();
        std::fs::write(pictures.join("face.png"), [0u8, 1, 2, 3]).unwrap();
        let mut snapshot = Snapshot::take(&root);
        let mut history = History::default();

        snapshot.keep_files_under(&root.join("npcs"));
        std::fs::remove_dir_all(root.join("npcs")).unwrap();
        let changes = snapshot.changes(&root);
        history.push("Delete npcs".into(), changes, Vec::new());

        let step = history.undo().unwrap().clone();
        assert!(apply(&step, true).is_empty());
        assert_eq!(
            read(&root.join("npcs").join("Gorbash.md")).as_deref(),
            Some("[[Saltmarsh]]")
        );
        assert_eq!(
            std::fs::read(pictures.join("face.png")).unwrap(),
            [0, 1, 2, 3]
        );

        snapshot.changes(&root);
        let step = history.redo().unwrap().clone();
        assert!(apply(&step, false).is_empty());
        assert!(!root.join("npcs").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edits_made_since_are_found() {
        let root = folder("since");
        let mut snapshot = Snapshot::take(&root);
        let mut history = History::default();
        std::fs::write(root.join("Saltmarsh.md"), "A port town").unwrap();
        history.push("Change".into(), snapshot.changes(&root), Vec::new());
        // Changed again in another app.
        std::fs::write(root.join("Saltmarsh.md"), "A ruined port town").unwrap();
        let step = history.undo().unwrap().clone();
        assert_eq!(changed_since(&step, true), vec![root.join("Saltmarsh.md")]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hidden_folders_and_unchanged_files_are_left_out() {
        let root = folder("hidden");
        let mut snapshot = Snapshot::take(&root);
        std::fs::create_dir_all(root.join(".dungeon_elements")).unwrap();
        std::fs::write(root.join(".dungeon_elements").join("tables.yaml"), "x").unwrap();
        // Saved again as it was.
        std::fs::write(root.join("Saltmarsh.md"), "A town").unwrap();
        assert!(snapshot.changes(&root).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_oldest_steps_go_first() {
        let mut history = History::default();
        for n in 0..MAX_STEPS + 5 {
            history.push(
                format!("Step {n}"),
                vec![Change {
                    path: PathBuf::from(format!("{n}.md")),
                    before: None,
                    after: Some(Content::File(Some(Arc::from(&b"x"[..])))),
                }],
                Vec::new(),
            );
        }
        assert_eq!(history.undo.len(), MAX_STEPS);
        assert_eq!(history.undo.front().unwrap().label, "Step 5");
        assert!(
            history
                .push("Nothing".into(), Vec::new(), Vec::new())
                .is_none()
        );
    }
}
