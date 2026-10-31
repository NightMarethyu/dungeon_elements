//! Internal links between notes, written the Obsidian way: `[[Note]]`, `[[Note|shown text]]`,
//! `[[Note#Heading]]`, `[[folder/Note]]`, and embeds like `![[Note]]`.
//!
//! A link names a note by its filename without `.md`, or by its path in the campaign when
//! two notes share a name. Renaming a note rewrites every link to it ([`relink_note`]), so
//! links keep working here and in Obsidian.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Color;
use iced::advanced::text;
use iced::widget::text_editor::{self, Action, Content, Cursor, Edit, Motion, Position};
use serde_yaml_ng::Value;

use crate::campaign;
use crate::note;

/// A `[[link]]` found in some text.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// The whole link, from `[[` (or the `!` of an embed) to `]]`.
    pub range: Range<usize>,
    /// The note's name or path: the text before any `#heading` or `|shown text`, trimmed.
    pub target: Range<usize>,
}

/// Finds the links in `text`, skipping code blocks and inline code.
pub fn find(text: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut in_code_block = false;
    let mut offset = 0;

    for line in text.split_inclusive('\n') {
        if is_fence(line) {
            in_code_block = !in_code_block;
        } else if !in_code_block {
            find_in_line(line, offset, &mut links);
        }
        offset += line.len();
    }

    links
}

/// Whether `line` opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// Adds the links in one line of text, which starts at byte `offset`, to `links`.
fn find_in_line(line: &str, offset: usize, links: &mut Vec<Link>) {
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Skip inline code: a run of backticks up to the next run of the same length.
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            let fence = &line[i..i + run];
            i += run;
            if let Some(end) = line[i..].find(fence) {
                i += end + run;
            }
            continue;
        }

        // Bytes, as `i` can be inside a letter like ö.
        if !bytes[i..].starts_with(b"[[") {
            i += 1;
            continue;
        }

        let inner_start = i + 2;
        let Some(length) = line[inner_start..].find("]]") else {
            break;
        };
        let inner = &line[inner_start..inner_start + length];
        if inner.contains("[[") || inner.contains('\n') {
            i = inner_start;
            continue;
        }

        // The target ends at a heading (`#`), or at the shown text (`|`, or `\|` inside a table).
        let mut target_end = inner.find(['#', '|']).unwrap_or(inner.len());
        if inner[..target_end].ends_with('\\') {
            target_end -= 1;
        }
        let raw = &inner[..target_end];
        let target_start = inner_start + (raw.len() - raw.trim_start().len());
        let target_end = inner_start + raw.trim_end().len();

        let end = inner_start + length + 2;
        if target_start < target_end {
            let start = if i > 0 && bytes[i - 1] == b'!' {
                i - 1
            } else {
                i
            };
            links.push(Link {
                range: offset + start..offset + end,
                target: offset + target_start..offset + target_end,
            });
        }
        i = end;
    }
}

/// A link target as [`Index`] compares it: lowercase, `/`-separated, without `.md`.
///
/// Each part is read as the file name a note with that name gets
/// ([`campaign::safe_name`]), so `[[Chapter 1: The Inn]]` finds `Chapter 1 - The Inn.md`.
pub fn normalize(target: &str) -> String {
    let target = target.trim().replace('\\', "/");
    let target = target.trim_start_matches('/');
    let target = match target.len().checked_sub(3) {
        Some(cut) if target.is_char_boundary(cut) && target[cut..].eq_ignore_ascii_case(".md") => {
            &target[..cut]
        }
        _ => target,
    };
    target
        .split('/')
        .map(|part| {
            let safe = campaign::safe_name(part);
            if safe.is_empty() {
                part.trim().to_string()
            } else {
                safe
            }
        })
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

/// Whether a word in `name` starts with `query`, like "inn" in "chapter 1 - the inn".
fn starts_word(name: &str, query: &str) -> bool {
    name.match_indices(query).any(|(at, _)| {
        name[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric())
    })
}

/// The campaign's notes, for finding the note a link points to.
#[derive(Debug, Clone, Default)]
pub struct Index {
    /// Shared, so the index is cheap to copy.
    notes: Arc<Vec<Entry>>,
    /// Every name a link can use to reach a note, as [`normalize`]d: each note's
    /// path in the campaign and every shorter path ending in its filename.
    names: Names,
    /// Other names notes go by (their `aliases`): `(normalized alias, note, alias)`.
    aliases: Arc<Vec<(String, usize, String)>>,
}

#[derive(Debug, Clone)]
struct Entry {
    path: PathBuf,
    /// The note's path in the campaign without `.md`, `/`-separated, like `npcs/Mira`.
    relative: String,
    /// `relative`, [`normalize`]d.
    key: String,
}

impl Entry {
    fn name(&self) -> &str {
        self.relative.rsplit('/').next().unwrap_or(&self.relative)
    }

    /// Whether a link to `target` (normalized) can mean this note.
    fn matches(&self, target: &str) -> bool {
        self.key == target
            || (self.key.ends_with(target)
                && self.key[..self.key.len() - target.len()].ends_with('/'))
    }
}

impl Index {
    /// Indexes the Markdown files in `notes`, which are inside the campaign folder `root`.
    pub fn new(root: &Path, notes: impl IntoIterator<Item = PathBuf>) -> Self {
        let notes: Vec<Entry> = notes
            .into_iter()
            .filter_map(|path| {
                let relative = path.strip_prefix(root).ok()?.with_extension("");
                let relative = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                Some(Entry {
                    key: normalize(&relative),
                    relative,
                    path,
                })
            })
            .collect();

        let mut names = HashSet::new();
        for entry in &notes {
            let mut key = entry.key.as_str();
            loop {
                names.insert(key.to_string());
                match key.split_once('/') {
                    Some((_, rest)) => key = rest,
                    None => break,
                }
            }
        }

        Index {
            notes: Arc::new(notes),
            names: Names(Arc::new(names)),
            aliases: Arc::default(),
        }
    }

    /// The index, where links can also use the other names in `aliases` (each note's
    /// `aliases`), as in Obsidian: `[[Jerry]]` finds the note BBEG if it goes by Jerry.
    pub fn with_aliases(mut self, aliases: &HashMap<PathBuf, Vec<String>>) -> Self {
        let mut found = Vec::new();
        for (index, entry) in self.notes.iter().enumerate() {
            for alias in aliases.get(&entry.path).into_iter().flatten() {
                let key = normalize(alias);
                if !key.is_empty() {
                    found.push((key, index, alias.trim().to_string()));
                }
            }
        }
        found.sort();

        let mut names: HashSet<String> = (*self.names.0).clone();
        names.extend(found.iter().map(|(key, _, _)| key.clone()));
        self.names = Names(Arc::new(names));
        self.aliases = Arc::new(found);
        self
    }

    pub fn names(&self) -> Names {
        self.names.clone()
    }

    /// The note a link to `target` in the note at `from` points to, if there is one.
    ///
    /// Like Obsidian: names are compared without case; when several notes match, an exact
    /// path wins, then a note in the same folder as `from`, then the one nearest the top.
    pub fn resolve(&self, target: &str, from: &Path) -> Option<&Path> {
        // A note's own name wins over another's alias.
        self.resolve_name(target, from).or_else(|| {
            let target = normalize(target);
            let folder = from.parent();
            self.aliases
                .iter()
                .filter(|(alias, _, _)| *alias == target)
                .map(|(_, index, _)| &self.notes[*index])
                .min_by_key(|entry| (entry.path.parent() != folder, entry.key.clone()))
                .map(|entry| entry.path.as_path())
        })
    }

    /// Like [`Self::resolve`], but only by the notes' file names, not their aliases.
    pub fn resolve_name(&self, target: &str, from: &Path) -> Option<&Path> {
        let target = normalize(target);
        if target.is_empty() {
            return None;
        }

        let folder = from.parent();
        self.notes
            .iter()
            .filter(|entry| entry.matches(&target))
            .min_by_key(|entry| {
                (
                    entry.key != target,
                    entry.path.parent() != folder,
                    entry.key.matches('/').count(),
                    entry.key.clone(),
                )
            })
            .map(|entry| entry.path.as_path())
    }

    /// The text to link to the note at `path` from the note at `from`: its name, or its
    /// path in the campaign if the name alone would point to another note (or `as_path` is set).
    pub fn link_text(&self, path: &Path, from: &Path, as_path: bool) -> Option<String> {
        let entry = self.notes.iter().find(|entry| entry.path == path)?;
        let name = entry.name();
        if !as_path && self.resolve(name, from) == Some(path) {
            Some(name.to_string())
        } else {
            Some(entry.relative.clone())
        }
    }

    /// Notes whose name, file name or alias contains `query`, best matches first:
    /// `(link text, label)` pairs. Notes inside `folder`, if given, come before the rest.
    ///
    /// Names and aliases are ranked together, so a name that starts with what was typed
    /// comes before one that only has it in the middle, whichever kind it is. The label
    /// shows the folder when two notes share a name.
    pub fn suggest(
        &self,
        query: &str,
        from: &Path,
        folder: Option<&Path>,
        limit: usize,
    ) -> Vec<(String, String)> {
        let query = normalize(query);
        // Each note by its file name (or its path, which the query may be typed as)...
        let names = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.key.contains(&query))
            .map(|(index, entry)| (index, normalize(entry.name()), None));
        // ...and by its other names. A name that only differs from the file's in its
        // punctuation is the same name, and shows on the file's suggestion instead.
        let aliases = self
            .aliases
            .iter()
            .filter(|(alias, index, _)| {
                alias.contains(&query) && *alias != normalize(self.notes[*index].name())
            })
            .map(|(alias, index, written)| (*index, alias.clone(), Some(written.as_str())));

        let mut matches: Vec<(usize, String, Option<&str>)> = names
            .chain(aliases)
            .filter(|(index, _, _)| self.notes[*index].path != from)
            .collect();
        matches.sort_by_cached_key(|(index, name, alias)| {
            let entry = &self.notes[*index];
            (
                folder.is_some_and(|folder| !entry.path.starts_with(folder)),
                !name.starts_with(&query),
                !starts_word(name, &query),
                !name.contains(&query),
                name.clone(),
                alias.is_some(),
                entry.key.clone(),
            )
        });

        matches
            .into_iter()
            .filter_map(|(index, _, alias)| {
                let entry = &self.notes[index];
                let link = self.link_text(&entry.path, from, false)?;
                Some(match alias {
                    // Obsidian links to an alias as `[[BBEG|Jerry]]`: the note, showing the alias.
                    Some(alias) => (
                        format!("{link}|{alias}"),
                        format!("{alias} ({})", self.label(entry)),
                    ),
                    // `Chapter 1 - The Inn.md`, called "Chapter 1: The Inn", is linked as
                    // `[[Chapter 1 - The Inn|Chapter 1: The Inn]]`, so it shows its real name.
                    None => match self.written_name(entry) {
                        Some(name) => (format!("{link}|{name}"), name.to_string()),
                        None => (link, self.label(entry)),
                    },
                })
            })
            .take(limit)
            .collect()
    }

    /// The notes inside `folder` (but not `from`), by name, as `(link text, label)` pairs
    /// like [`Self::suggest`]'s.
    pub fn in_folder(&self, folder: &Path, from: &Path) -> Vec<(String, String)> {
        let mut entries: Vec<&Entry> = self
            .notes
            .iter()
            .filter(|entry| entry.path.starts_with(folder) && entry.path != from)
            .collect();
        entries.sort_by_cached_key(|entry| normalize(entry.name()));
        entries
            .into_iter()
            .filter_map(|entry| {
                let link = self.link_text(&entry.path, from, false)?;
                Some(match self.written_name(entry) {
                    Some(name) => (format!("{link}|{name}"), name.to_string()),
                    None => (link, self.label(entry)),
                })
            })
            .collect()
    }

    /// [`Self::written_name`] of the note at `path`.
    pub fn written_name_of(&self, path: &Path) -> Option<&str> {
        let entry = self.notes.iter().find(|entry| entry.path == path)?;
        self.written_name(entry)
    }

    /// The name `entry` is written with, if its file name had to leave out characters:
    /// the alias that reads as its file name (`Chapter 1: The Inn` for `Chapter 1 - The Inn`).
    fn written_name(&self, entry: &Entry) -> Option<&str> {
        let name = normalize(entry.name());
        self.aliases
            .iter()
            .find(|(alias, index, written)| {
                self.notes[*index].path == entry.path && *alias == name && written != entry.name()
            })
            .map(|(_, _, written)| written.as_str())
    }

    /// How to show `entry` in a list of notes: its name, or its path when two notes share
    /// the name.
    fn label(&self, entry: &Entry) -> String {
        let shared = self
            .notes
            .iter()
            .filter(|other| other.name().eq_ignore_ascii_case(entry.name()))
            .count()
            > 1;
        if shared {
            entry.relative.clone()
        } else {
            entry.name().to_string()
        }
    }

    /// Notes whose name is spelled almost like `target`, a link that points nowhere, for
    /// offering what it may have meant: `(link text, label)` pairs, closest first.
    pub fn similar(&self, target: &str, from: &Path, limit: usize) -> Vec<(String, String)> {
        let target = normalize(target);
        let wanted = target.rsplit('/').next().unwrap_or(&target);
        let wanted: Vec<char> = wanted.chars().collect();
        // Short names need a closer match, or every three-letter name would look alike.
        let most = match wanted.len() {
            0..=4 => 1,
            5..=11 => 2,
            _ => 3,
        };

        let mut matches: Vec<(usize, &Entry)> = self
            .notes
            .iter()
            .filter(|entry| entry.path != from)
            .filter_map(|entry| {
                let name: Vec<char> = entry.name().to_lowercase().chars().collect();
                let distance = edit_distance(&wanted, &name);
                (distance <= most).then_some((distance, entry))
            })
            .collect();
        matches.sort_by_key(|(distance, entry)| (*distance, entry.key.clone()));

        matches
            .into_iter()
            .take(limit)
            .filter_map(|(_, entry)| {
                Some((self.link_text(&entry.path, from, false)?, self.label(entry)))
            })
            .collect()
    }
}

/// How many letters must be added, removed or changed to turn `a` into `b`.
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(x != y))
                .min(above + 1)
                .min(row[j] + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The names notes can be linked by, for coloring links that point nowhere.
///
/// Two are equal only if they are the same set, so the editor's highlighter can
/// cheaply tell when the campaign changed.
#[derive(Debug, Clone, Default)]
pub struct Names(Arc<HashSet<String>>);

impl PartialEq for Names {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Names {
    pub fn contains(&self, target: &str) -> bool {
        self.0.contains(&normalize(target))
    }
}

/// Notes that were renamed or moved, as `(old, new)` paths: what changed, for rewriting
/// the links to them. Renaming a folder moves every note in it.
pub struct Rename<'a> {
    pub moved: &'a [(PathBuf, PathBuf)],
    /// The campaign's notes before and after the rename.
    pub before: &'a Index,
    pub after: &'a Index,
}

impl Rename<'_> {
    /// Where the note now at `path` was before the rename.
    fn was(&self, path: &Path) -> PathBuf {
        self.moved
            .iter()
            .find(|(_, new)| new == path)
            .map_or_else(|| path.to_path_buf(), |(old, _)| old.clone())
    }

    /// Where the note that was at `path` before the rename is now.
    fn now<'b>(&'b self, path: &'b Path) -> &'b Path {
        self.moved
            .iter()
            .find(|(old, _)| old == path)
            .map_or(path, |(_, new)| new.as_path())
    }
}

/// What a link to `target` in the note at `from` should say after `rename`, so it points
/// where it did before, if that changes.
///
/// Links to the renamed note get its new name. A link to another note is rewritten as a
/// path only if the rename would otherwise make it point to the renamed note instead.
fn relinked(target: &str, from: &Path, rename: &Rename) -> Option<String> {
    let meant = rename.before.resolve(target, &rename.was(from))?;
    let meant = rename.now(meant);
    // Only by name: Obsidian doesn't follow a link to an alias.
    if rename.after.resolve_name(target, from) == Some(meant) {
        return None;
    }

    let as_path = target.contains('/') || target.contains('\\');
    let mut replacement = rename.after.link_text(meant, from, as_path)?;
    if target.to_lowercase().ends_with(".md") {
        replacement.push_str(".md");
    }
    // A note whose name has characters its file name can't shows that name.
    if let Some(name) = rename.after.written_name_of(meant) {
        replacement = format!("{replacement}|{name}");
    }
    Some(replacement)
}

/// Rewrites the links in a whole note, the note at `from`, front matter included, so they
/// point where they did before `rename`. Headings, shown text and `!` embeds are kept.
/// Returns the new text and how many links changed, or `None` if none did.
pub fn relink_note(text: &str, from: &Path, rename: &Rename) -> Option<(String, usize)> {
    rewrite_note(text, &mut |target| relinked(target, from, rename))
}

/// Points the links to `old` in a whole note to `new` instead, keeping headings, shown text
/// and embeds. Returns `None` if no link named `old`.
pub fn retarget_note(text: &str, old: &str, new: &str) -> Option<(String, usize)> {
    let old = normalize(old);
    rewrite_note(text, &mut |target| {
        (normalize(target) == old).then(|| new.to_string())
    })
}

/// Changes the heading the links in `text` (the note at `from`) point to from `old` to
/// `new`, in links to the note at `note`, like `[[Chapter 4#Encounters]]`. Returns the new text and how many links
/// changed, or `None` if none did.
pub fn rename_heading(
    text: &str,
    from: &Path,
    index: &Index,
    note: &Path,
    old: &str,
    new: &str,
) -> Option<(String, usize)> {
    let mut result = text.to_string();
    let mut changed = 0;
    for link in find(text).into_iter().rev() {
        if index.resolve(text[link.target.clone()].trim(), from) != Some(note) {
            continue;
        }
        // The heading runs from the `#` after the target to the shown text or the end.
        let close = link.range.end - 2;
        let tail = &text[link.target.end..close];
        let Some(rest) = tail.trim_start().strip_prefix('#') else {
            continue;
        };
        let start = close - tail.len() + (tail.len() - rest.len());
        let end = start
            + rest
                .find("\\|")
                .or_else(|| rest.find('|'))
                .unwrap_or(rest.len());
        if text[start..end].trim().eq_ignore_ascii_case(old.trim()) {
            result.replace_range(start..end, new.trim());
            changed += 1;
        }
    }
    (changed > 0).then_some((result, changed))
}

/// Turns the links to `target` in `text` back into plain text: their shown text, or the
/// name they link to. Returns `None` if no link named `target`.
pub fn unlink_all(text: &str, target: &str) -> Option<String> {
    let target = normalize(target);
    let mut result = text.to_string();
    let mut changed = false;

    let yaml = front_matter_len(text);
    for link in find(text).into_iter().rev() {
        if normalize(&read_target(text, &link, link.range.start < yaml)) != target {
            continue;
        }
        let inner = text[link.range.clone()]
            .trim_start_matches('!')
            .trim_start_matches("[[")
            .trim_end_matches("]]");
        let shown = match inner.split_once('|') {
            Some((_, shown)) => shown,
            None => &text[link.target.clone()],
        };
        result.replace_range(link.range.clone(), shown.trim());
        changed = true;
    }

    changed.then_some(result)
}

/// Rewrites the target of each link in `text` that `new_target` gives a new one for.
/// Returns the new text and how many links changed.
fn rewrite(text: &str, new_target: &mut dyn FnMut(&str) -> Option<String>) -> (String, usize) {
    let mut result = text.to_string();
    let mut changed = 0;

    // From the end, so earlier ranges stay valid.
    for link in find(text).into_iter().rev() {
        let old = &text[link.target.clone()];
        if let Some(replacement) = new_target(old) {
            retarget(&mut result, text, &link, old, &replacement, Quote::Plain);
            changed += 1;
        }
    }

    (result, changed)
}

/// Points `link`, found in `text`, at `replacement` in `result` (a copy of `text` that
/// is only changed after the link). `old` is where it pointed.
///
/// A replacement like `Note|Shown` also sets the text the link shows, if it showed none
/// or showed the old note's name, but not if it shows words of its own.
fn retarget(
    result: &mut String,
    text: &str,
    link: &Link,
    old: &str,
    replacement: &str,
    quote: Quote,
) {
    let (target, shown) = match replacement.split_once('|') {
        Some((target, shown)) => (target, Some(shown)),
        None => (replacement, None),
    };

    if let Some(shown) = shown {
        let close = link.range.end - 2;
        let tail = &text[link.target.end..close];
        match tail.find('|') {
            Some(pipe) => {
                let range = link.target.end + pipe + 1..close;
                let current = unquote(&text[range.clone()], quote);
                if normalize(&current) == normalize(old) {
                    result.replace_range(range, &requote(shown, quote));
                }
            }
            None => {
                // In a table, a bare | would end the cell.
                let line_start = text[..link.range.start].rfind('\n').map_or(0, |i| i + 1);
                let pipe = if text[line_start..].trim_start().starts_with('|') {
                    "\\|"
                } else {
                    "|"
                };
                result.insert_str(close, &format!("{pipe}{}", requote(shown, quote)));
            }
        }
    }
    result.replace_range(link.target.clone(), &requote(target, quote));
}

/// Like [`rewrite`], for a whole note, front matter included. Returns `None` if no link changed.
///
/// Links in the front matter are YAML strings like `owner: '[[Mira]]'`. If a new name
/// can't be put in place as it is (say, `Mira's Cat` in single quotes), the front matter
/// is written out again so it still holds the right values.
fn rewrite_note(
    text: &str,
    new_target: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<(String, usize)> {
    let (front, body) = note::split_front_matter(text).ok()?;
    let head = &text[..text.len() - body.len()];

    let (new_body, body_changes) = rewrite(body, new_target);
    let Some(yaml) = front else {
        return (body_changes > 0).then(|| (format!("{head}{new_body}"), body_changes));
    };

    let (new_yaml, front_changes) = rewrite_yaml(yaml, new_target);
    if front_changes + body_changes == 0 {
        return None;
    }
    if front_changes == 0 {
        return Some((format!("{head}{new_body}"), body_changes));
    }

    // Check that the front matter still reads back as the values it should have.
    let expected = serde_yaml_ng::from_str::<Value>(yaml)
        .ok()
        .map(|value| rewrite_value(value, new_target));
    let actual = serde_yaml_ng::from_str::<Value>(&new_yaml).ok();

    let new_head = match expected {
        Some(expected) if actual.as_ref() != Some(&expected) => {
            let yaml = serde_yaml_ng::to_string(&expected).ok()?;
            format!("---\n{yaml}---\n")
        }
        _ => head.replacen(yaml, &new_yaml, 1),
    };

    Some((
        format!("{new_head}{new_body}"),
        front_changes + body_changes,
    ))
}

/// Like [`rewrite`], for front matter YAML: each target is read as the YAML string it is
/// part of (`'[[John''s Shop]]'` links to John's Shop), and a new one is written so it
/// stays valid there (John's Shop becomes `John''s Shop` in single quotes).
fn rewrite_yaml(yaml: &str, new_target: &mut dyn FnMut(&str) -> Option<String>) -> (String, usize) {
    let mut result = yaml.to_string();
    let mut changed = 0;

    for link in find(yaml).into_iter().rev() {
        let quote = quote_at(yaml, link.target.start);
        let old = unquote(&yaml[link.target.clone()], quote);
        if let Some(replacement) = new_target(&old) {
            retarget(&mut result, yaml, &link, &old, &replacement, quote);
            changed += 1;
        }
    }

    (result, changed)
}

/// The note `link` (found in `text`) points to, read as YAML if it is in the front matter.
fn read_target(text: &str, link: &Link, in_yaml: bool) -> String {
    let raw = &text[link.target.clone()];
    if in_yaml {
        unquote(raw, quote_at(text, link.target.start))
    } else {
        raw.to_string()
    }
}

/// The length of the front matter at the start of `text`, closing `---` included, or 0.
fn front_matter_len(text: &str) -> usize {
    match note::split_front_matter(text) {
        Ok((Some(_), body)) => text.len() - body.len(),
        _ => 0,
    }
}

/// The notes the `[[links]]` in a whole note point to, in order. Links in the front
/// matter are read as the YAML strings they are part of.
pub fn targets(text: &str) -> Vec<String> {
    let yaml = front_matter_len(text);
    find(text)
        .iter()
        .map(|link| read_target(text, link, link.range.start < yaml))
        .collect()
}

/// How the YAML at a spot in a line is quoted.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Quote {
    Plain,
    Single,
    Double,
}

/// How byte `at` of `yaml` is quoted, going by its line up to there.
fn quote_at(yaml: &str, at: usize) -> Quote {
    let line_start = yaml[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &yaml.as_bytes()[line_start..at];
    let mut quote = Quote::Plain;
    let mut i = 0;

    while i < line.len() {
        let b = line[i];
        match quote {
            Quote::Plain => {
                // A quote only starts a string where a value starts, not in `John's`.
                let starts_value = line[..i]
                    .iter()
                    .rev()
                    .find(|b| !b.is_ascii_whitespace())
                    .is_none_or(|b| b":-[{,?".contains(b));
                if starts_value && b == b'\'' {
                    quote = Quote::Single;
                } else if starts_value && b == b'"' {
                    quote = Quote::Double;
                }
            }
            Quote::Single if b == b'\'' => {
                if line.get(i + 1) == Some(&b'\'') {
                    i += 1;
                } else {
                    quote = Quote::Plain;
                }
            }
            Quote::Double if b == b'\\' => i += 1,
            Quote::Double if b == b'"' => quote = Quote::Plain,
            _ => {}
        }
        i += 1;
    }
    quote
}

/// The text a piece of YAML quoted with `quote` stands for.
fn unquote(raw: &str, quote: Quote) -> String {
    match quote {
        Quote::Plain => raw.to_string(),
        Quote::Single => raw.replace("''", "'"),
        Quote::Double => {
            let mut out = String::with_capacity(raw.len());
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                match (c, chars.clone().next()) {
                    ('\\', Some(next @ ('"' | '\\' | '/'))) => {
                        out.push(next);
                        chars.next();
                    }
                    _ => out.push(c),
                }
            }
            out
        }
    }
}

/// `text` written so it reads back as itself inside YAML quoted with `quote`.
fn requote(text: &str, quote: Quote) -> String {
    match quote {
        Quote::Plain => text.to_string(),
        Quote::Single => text.replace('\'', "''"),
        Quote::Double => text.replace('\\', "\\\\").replace('"', "\\\""),
    }
}

/// Whether line `line` of `content` is inside the front matter at the top of the note.
fn in_front_matter(content: &Content, line: usize) -> bool {
    let is_fence = |index: usize| {
        content
            .line(index)
            .is_some_and(|line| matches!(line.text.trim_end(), "---" | "..."))
    };
    line > 0 && is_fence(0) && !(1..=line).any(is_fence)
}

/// Rewrites the links in the strings of a YAML value.
fn rewrite_value(value: Value, new_target: &mut dyn FnMut(&str) -> Option<String>) -> Value {
    match value {
        Value::String(text) => Value::String(rewrite(&text, new_target).0),
        // `owner: [[Mira]]` without quotes reads as a list in a list.
        Value::Sequence(outer)
            if matches!(outer.as_slice(), [Value::Sequence(inner)]
                if matches!(inner.as_slice(), [Value::String(_)])) =>
        {
            let [Value::Sequence(inner)] = outer.as_slice() else {
                unreachable!()
            };
            let [Value::String(target)] = inner.as_slice() else {
                unreachable!()
            };
            let target = new_target(target.trim()).unwrap_or_else(|| target.clone());
            Value::Sequence(vec![Value::Sequence(vec![Value::String(target)])])
        }
        Value::Sequence(items) => Value::Sequence(
            items
                .into_iter()
                .map(|item| rewrite_value(item, new_target))
                .collect(),
        ),
        Value::Mapping(mapping) => Value::Mapping(
            mapping
                .into_iter()
                .map(|(key, value)| (key, rewrite_value(value, new_target)))
                .collect(),
        ),
        other => other,
    }
}

// Typing links in a text editor.

/// The `[[` before the cursor that isn't closed yet, and the text typed after it.
fn open_link(content: &Content) -> Option<(Position, String)> {
    let cursor = content.cursor();
    if cursor
        .selection
        .is_some_and(|selection| selection != cursor.position)
    {
        return None;
    }
    let position = cursor.position;
    let line = content.line(position.line)?.text;
    let before = line.get(..position.column)?;

    let start = before.rfind("[[")?;
    let typed = &before[start + 2..];
    if typed.contains(['[', ']', '|', '#', '`']) {
        return None;
    }
    // In the middle of a finished link's name, there is nothing to suggest.
    let after = &line[position.column..];
    if let Some(close) = after.find("]]")
        && !after[..close].contains("[[")
        && !after[..close].is_empty()
        && !after.starts_with(['|', '#'])
    {
        return None;
    }
    Some((
        Position {
            line: position.line,
            column: start + 2,
        },
        typed.to_string(),
    ))
}

/// What has been typed after an unclosed `[[` before the cursor, if the cursor is in one.
pub fn query(content: &Content) -> Option<String> {
    open_link(content).map(|(_, typed)| typed)
}

/// Finishes the link being typed at the cursor with `link`, and moves past its `]]`.
pub fn complete(content: &mut Content, link: &str) {
    let Some((start, _)) = open_link(content) else {
        return;
    };
    let end = content.cursor().position;
    let line = content.line(end.line).map(|line| line.text.into_owned());
    let after = line
        .as_deref()
        .and_then(|line| line.get(end.column..))
        .unwrap_or("");

    // In the front matter, the name goes inside a YAML string, and must be escaped to fit.
    let link = match &line {
        Some(line) if in_front_matter(content, end.line) => {
            requote(link, quote_at(line, start.column.min(line.len())))
        }
        _ => link.to_string(),
    };
    let link = link.as_str();

    content.move_to(Cursor {
        position: end,
        selection: Some(start),
    });

    if after.starts_with("]]") {
        content.perform(Action::Edit(Edit::Paste(Arc::new(link.to_string()))));
        content.perform(Action::Move(Motion::Right));
        content.perform(Action::Move(Motion::Right));
    } else if after.starts_with(['|', '#']) {
        content.perform(Action::Edit(Edit::Paste(Arc::new(link.to_string()))));
    } else {
        content.perform(Action::Edit(Edit::Paste(Arc::new(format!("{link}]]")))));
    }
}

/// The target of the link the cursor is in, if it is in one.
pub fn under_cursor(content: &Content) -> Option<String> {
    let position = content.cursor().position;
    let line = content.line(position.line)?.text;

    let mut links = Vec::new();
    find_in_line(&line, 0, &mut links);
    let link = links
        .into_iter()
        .find(|link| link.range.start <= position.column && position.column <= link.range.end)?;
    Some(read_target(
        &line,
        &link,
        in_front_matter(content, position.line),
    ))
}

/// Applies `action` to `content`, closing a typed `[[` with `]]` the way Obsidian does
/// (and typing over that `]]` instead of adding another), and carrying lists on when
/// Enter is pressed in one.
pub fn perform(content: &mut Content, action: Action) {
    let cursor = content.cursor();
    let caret = cursor
        .selection
        .is_none_or(|selection| selection == cursor.position);
    let line = content
        .line(cursor.position.line)
        .map(|line| line.text.into_owned())
        .unwrap_or_default();
    let before = line.get(..cursor.position.column).unwrap_or("");
    let after = line.get(cursor.position.column..).unwrap_or("");
    // Whether the cursor is after a `[[` whose `]]` it hasn't passed yet.
    let in_open_link = before
        .rfind("[[")
        .is_some_and(|start| !before[start..].contains("]]"));

    match action {
        Action::Edit(Edit::Enter) if crate::format::continue_list(content) => {}
        Action::Edit(Edit::Insert('['))
            if caret && before.ends_with('[') && !after.starts_with(']') =>
        {
            content.perform(action);
            content.perform(Action::Edit(Edit::Paste(Arc::new("]]".into()))));
            content.perform(Action::Move(Motion::Left));
            content.perform(Action::Move(Motion::Left));
        }
        Action::Edit(Edit::Insert(']')) if caret && after.starts_with(']') && in_open_link => {
            content.perform(Action::Move(Motion::Right));
        }
        action => content.perform(action),
    }
}

/// Wraps the selected text in `[[` and `]]`, or starts an empty link at the cursor.
pub fn insert(content: &mut Content) {
    match content.selection().filter(|text| !text.contains('\n')) {
        Some(selected) => {
            content.perform(Action::Edit(Edit::Paste(Arc::new(format!(
                "[[{selected}]]"
            )))));
        }
        None => {
            content.perform(Action::Edit(Edit::Paste(Arc::new("[[]]".into()))));
            content.perform(Action::Move(Motion::Left));
            content.perform(Action::Move(Motion::Left));
        }
    }
}

// Coloring links in a text editor.

/// How [`Highlighter`] colors links and Markdown marks.
#[derive(Debug, Clone, PartialEq)]
pub struct Colors {
    /// Links to notes that exist.
    pub link: Color,
    /// Links to notes that don't exist yet.
    pub missing: Color,
    /// Markdown marks like `#` and `**`, and the brackets of links, dimmer than the words.
    pub mark: Color,
    /// Whether links are bold too, for themes where their color is the text's.
    pub bold: bool,
    pub names: Names,
}

/// Colors the `[[links]]` in a text editor, and dims the Markdown marks around the words.
pub struct Highlighter {
    colors: Colors,
    /// Where each line, up to the next one to color, starts.
    starts: Vec<Place>,
}

/// Where a line of a note is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Place {
    /// The first line, which may open the front matter.
    Top,
    FrontMatter,
    Text,
    Code,
}

impl text::Highlighter for Highlighter {
    type Settings = Colors;
    /// The link's color, and whether it is bold.
    type Highlight = (Color, bool);
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, (Color, bool))>;

    fn new(colors: &Colors) -> Self {
        Highlighter {
            colors: colors.clone(),
            starts: vec![Place::Top],
        }
    }

    fn update(&mut self, colors: &Colors) {
        self.colors = colors.clone();
        self.starts.truncate(1);
    }

    fn change_line(&mut self, line: usize) {
        self.starts.truncate(line + 1);
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let place = *self.starts.last().unwrap_or(&Place::Text);
        let yaml_fence = matches!(line.trim_end(), "---" | "...");
        let (here, next) = match place {
            Place::Top if line.trim_end() == "---" => (Place::Top, Place::FrontMatter),
            Place::FrontMatter if yaml_fence => (Place::Top, Place::Text),
            Place::FrontMatter => (Place::FrontMatter, Place::FrontMatter),
            Place::Code => (
                Place::Code,
                if is_fence(line) {
                    Place::Text
                } else {
                    Place::Code
                },
            ),
            Place::Top | Place::Text if is_fence(line) => (Place::Code, Place::Code),
            Place::Top | Place::Text => (Place::Text, Place::Text),
        };
        self.starts.push(next);

        let mark = (self.colors.mark, false);
        let line_end = line.trim_end_matches(['\n', '\r']).len();
        // Fences around front matter and code are marks from end to end.
        if here == Place::Top || (here == Place::Code && is_fence(line)) {
            return vec![(0..line_end, mark)].into_iter();
        }
        if here == Place::Code {
            return Vec::new().into_iter();
        }

        let mut links = Vec::new();
        find_in_line(line, 0, &mut links);

        let mut spans = Vec::new();
        for link in &links {
            let target = read_target(line, link, here == Place::FrontMatter);
            let color = if self.colors.names.contains(&target) {
                self.colors.link
            } else {
                self.colors.missing
            };
            let range = link.range.clone();
            let open = if line[range.clone()].starts_with('!') {
                3
            } else {
                2
            };
            spans.push((range.start..range.start + open, mark));
            spans.push((range.start + open..range.end - 2, (color, self.colors.bold)));
            spans.push((range.end - 2..range.end, mark));
        }

        if here == Place::Text {
            // Marks inside a link (pulldown-cmark doesn't know them) are left to it.
            for range in crate::marks::find(line) {
                let mut start = range.start;
                for link in &links {
                    if link.range.end <= start || link.range.start >= range.end {
                        continue;
                    }
                    if link.range.start > start {
                        spans.push((start..link.range.start, mark));
                    }
                    start = start.max(link.range.end);
                }
                if start < range.end {
                    spans.push((start..range.end, mark));
                }
            }
            spans.sort_by_key(|(range, _)| range.start);
        }

        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.starts.len() - 1
    }
}

/// Turns a [`Highlighter`] highlight into a text format.
pub fn format(
    &(color, bold): &(Color, bool),
    _theme: &iced::Theme,
) -> text::highlighter::Format<iced::Font> {
    text::highlighter::Format {
        color: Some(color),
        font: bold.then_some(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::DEFAULT
        }),
    }
}

/// The key binding a link-aware editor adds: Tab finishes the link being typed with
/// `accept`, if there is a suggestion.
pub fn tab_binding<M: Clone>(
    key_press: &text_editor::KeyPress,
    accept: &Option<M>,
) -> Option<text_editor::Binding<M>> {
    let tab = matches!(
        key_press.key,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab)
    );
    let focused = matches!(key_press.status, text_editor::Status::Focused { .. });
    match accept {
        Some(message) if tab && focused && key_press.modifiers.is_empty() => {
            Some(text_editor::Binding::Custom(message.clone()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(text: &str) -> Vec<&str> {
        find(text)
            .into_iter()
            .map(|link| &text[link.target])
            .collect()
    }

    #[test]
    fn renames_the_heading_links_point_to() {
        let root = Path::new("/c");
        let index = Index::new(root, [root.join("Chapter 4.md"), root.join("Other.md")]);
        let note = root.join("Chapter 4.md");
        let text = "[[Chapter 4#Encounters]] [[chapter 4 # encounters|roll]] [[#Encounters]] [[Other#Encounters]] [[Chapter 4#Weather]] | [[Chapter 4#Encounters\\|x]] |";
        let (renamed, count) = rename_heading(
            text,
            &root.join("Other.md"),
            &index,
            &note,
            "Encounters",
            "On the road",
        )
        .unwrap();
        assert_eq!(count, 3);
        assert_eq!(
            renamed,
            "[[Chapter 4#On the road]] [[chapter 4 #On the road|roll]] [[#Encounters]] [[Other#Encounters]] [[Chapter 4#Weather]] | [[Chapter 4#On the road\\|x]] |"
        );
        let (own, count) =
            rename_heading(text, &note, &index, &note, "Encounters", "Road").unwrap();
        assert_eq!(count, 3);
        assert!(own.contains("[[Chapter 4#Road]]"));
    }

    #[test]
    fn finds_links() {
        let text = "See [[Mira]], [[npcs/Bob|the baker]] and ![[map]].\n[[ Spaced #Heading ]] [[Table\\|alias]] [[#Own heading]] [[]] [[open";
        assert_eq!(
            targets(text),
            ["Mira", "npcs/Bob", "map", "Spaced", "Table"]
        );

        let links = find(text);
        assert_eq!(&text[links[2].range.clone()], "![[map]]");
        assert_eq!(&text[links[1].range.clone()], "[[npcs/Bob|the baker]]");
    }

    #[test]
    fn finds_links_among_any_letters() {
        let text = "Café Ælf [[Jön's Lóng 🧦]] — Дракон [[Дракон|змей]] `ö [[no]]` 日本 [[東京]]";
        assert_eq!(targets(text), ["Jön's Lóng 🧦", "Дракон", "東京"]);
    }

    #[test]
    fn skips_code() {
        let text = "`[[not]]` and ``a ` [[nor]]`` but [[yes]]\n```\n[[code]]\n```\n~~~\n[[tilde]]\n~~~\n[[after]]";
        assert_eq!(targets(text), ["yes", "after"]);
    }

    fn campaign(root: &Path, notes: &[&str]) -> Index {
        Index::new(
            root,
            notes.iter().map(|note| root.join(format!("{note}.md"))),
        )
    }

    #[test]
    fn resolves_like_obsidian() {
        let root = Path::new("/c");
        let index = campaign(
            root,
            &["npcs/Mira", "npcs/Bob", "lore/Bob", "Bob", "lore/deep/Tide"],
        );
        let from_npcs = root.join("npcs/Mira.md");
        let from_lore = root.join("lore/Tide.md");

        assert_eq!(
            index.resolve("mira", &from_lore),
            Some(root.join("npcs/Mira.md").as_path())
        );
        assert_eq!(
            index.resolve("Mira.md", &from_lore),
            Some(root.join("npcs/Mira.md").as_path())
        );
        assert_eq!(
            index.resolve("deep/tide", &from_lore),
            Some(root.join("lore/deep/Tide.md").as_path())
        );
        assert_eq!(index.resolve("eep/Tide", &from_lore), None);
        assert_eq!(index.resolve("Nobody", &from_lore), None);

        // An exact path wins, then the same folder, then the shortest path.
        assert_eq!(
            index.resolve("Bob", &from_lore),
            Some(root.join("Bob.md").as_path())
        );
        assert_eq!(
            index.resolve("lore/Bob", &from_npcs),
            Some(root.join("lore/Bob.md").as_path())
        );
        let index = campaign(root, &["npcs/Bob", "lore/Bob"]);
        assert_eq!(
            index.resolve("Bob", &from_npcs),
            Some(root.join("npcs/Bob.md").as_path())
        );

        assert_eq!(
            index
                .link_text(&root.join("npcs/Bob.md"), &from_npcs, false)
                .as_deref(),
            Some("Bob")
        );
        assert_eq!(
            index
                .link_text(&root.join("lore/Bob.md"), &from_npcs, false)
                .as_deref(),
            Some("lore/Bob")
        );

        assert!(index.names().contains("NPCS/bob.md"));
        assert!(index.names().contains("bob"));
        assert!(!index.names().contains("pcs/bob"));
    }

    #[test]
    fn suggests_names() {
        let root = Path::new("/c");
        let index = campaign(
            root,
            &[
                "npcs/Mira",
                "npcs/Admiral",
                "lore/Mirage",
                "items/Bob",
                "npcs/Bob",
            ],
        );
        let from = root.join("journal/Day 1.md");

        let suggestions = index.suggest("mir", &from, None, 8);
        assert_eq!(
            suggestions,
            [
                ("Mira".into(), "Mira".into()),
                ("Mirage".into(), "Mirage".into()),
                ("Admiral".into(), "Admiral".into()),
            ]
        );
        let bobs = index.suggest("bob", &from, None, 8);
        assert_eq!(bobs[0].1, "items/Bob");
        assert_eq!(bobs[1].1, "npcs/Bob");

        // A link field that links to NPCs lists them first.
        let bobs = index.suggest("bob", &from, Some(&root.join("npcs")), 8);
        assert_eq!(bobs[0].1, "npcs/Bob");
    }

    fn rename_in(
        root: &Path,
        notes: &[&str],
        old: &str,
        new: &str,
        text: &str,
        from: &str,
    ) -> Option<(String, usize)> {
        let old_path = root.join(format!("{old}.md"));
        let new_path = root.join(format!("{new}.md"));
        let before = campaign(root, notes);
        let after_notes: Vec<&str> = notes
            .iter()
            .map(|note| if *note == old { new } else { note })
            .collect();
        let after = campaign(root, &after_notes);
        let moved = [(old_path, new_path)];
        let rename = Rename {
            moved: &moved,
            before: &before,
            after: &after,
        };
        let from = root.join(format!("{from}.md"));
        relink_note(text, &from, &rename)
    }

    #[test]
    fn suggests_names_and_aliases_together() {
        let root = Path::new("/c");
        let aliases = HashMap::from([
            (root.join("npcs/BBEG.md"), vec!["Jerry".to_string()]),
            (root.join("lore/Old Inn.md"), vec!["The Inn".to_string()]),
        ]);
        let index = campaign(
            root,
            &[
                "npcs/BBEG",
                "npcs/Bijerry",
                "npcs/Jerrod",
                "lore/Old Inn",
                "lore/Spinning",
            ],
        )
        .with_aliases(&aliases);
        let from = root.join("Day.md");

        // An alias that starts with what was typed comes before a name that only has it
        // in the middle, and isn't crowded out by them.
        assert_eq!(
            index.suggest("jer", &from, None, 2),
            [
                ("Jerrod".to_string(), "Jerrod".to_string()),
                ("BBEG|Jerry".to_string(), "Jerry (BBEG)".to_string()),
            ]
        );
        // A word starting with it comes before the middle of a word.
        assert_eq!(
            index.suggest("inn", &from, None, 5),
            [
                ("Old Inn".to_string(), "Old Inn".to_string()),
                (
                    "Old Inn|The Inn".to_string(),
                    "The Inn (Old Inn)".to_string()
                ),
                ("Spinning".to_string(), "Spinning".to_string()),
            ]
        );
    }

    #[test]
    fn links_can_use_aliases() {
        let root = Path::new("/c");
        let aliases = HashMap::from([
            (root.join("npcs/BBEG.md"), vec!["Jerry".to_string()]),
            (
                root.join("lore/Chapter 1 - The Inn.md"),
                vec!["Chapter 1: The Inn".to_string()],
            ),
        ]);
        let index = campaign(
            root,
            &["npcs/BBEG", "npcs/Jeremy", "lore/Chapter 1 - The Inn"],
        )
        .with_aliases(&aliases);
        let from = root.join("Day.md");

        assert_eq!(
            index.resolve("jerry", &from),
            Some(root.join("npcs/BBEG.md").as_path())
        );
        assert!(index.names().contains("Jerry"));
        assert_eq!(
            index.suggest("jer", &from, None, 5),
            [
                ("Jeremy".to_string(), "Jeremy".to_string()),
                ("BBEG|Jerry".to_string(), "Jerry (BBEG)".to_string()),
            ]
        );
        assert_eq!(
            index.suggest("chapter", &from, None, 5),
            [(
                "Chapter 1 - The Inn|Chapter 1: The Inn".to_string(),
                "Chapter 1: The Inn".to_string()
            )]
        );
    }

    #[test]
    fn renaming_a_note_with_punctuation_updates_the_name_links_show() {
        let root = Path::new("/c");
        let before = campaign(root, &["Chapter 1 - The Inn", "Day"]);
        let after = campaign(root, &["Chapter 2 - Docks", "Day"]).with_aliases(&HashMap::from([(
            root.join("Chapter 2 - Docks.md"),
            vec!["Chapter 2: Docks".to_string()],
        )]));
        let moved = [(
            root.join("Chapter 1 - The Inn.md"),
            root.join("Chapter 2 - Docks.md"),
        )];
        let rename = Rename {
            moved: &moved,
            before: &before,
            after: &after,
        };
        let text = "---\nwhere: '[[Chapter 1 - The Inn]]'\n---\n\n[[Chapter 1 - The Inn|Chapter 1: The Inn]], [[Chapter 1 - The Inn|that night]]\n\n| a | [[Chapter 1 - The Inn]] |\n";
        let (new, count) = relink_note(text, &root.join("Day.md"), &rename).unwrap();
        assert_eq!(count, 4);
        assert_eq!(
            new,
            "---\nwhere: '[[Chapter 2 - Docks|Chapter 2: Docks]]'\n---\n\n[[Chapter 2 - Docks|Chapter 2: Docks]], [[Chapter 2 - Docks|that night]]\n\n| a | [[Chapter 2 - Docks\\|Chapter 2: Docks]] |\n"
        );
    }

    #[test]
    fn renaming_rewrites_links() {
        let root = Path::new("/c");
        let notes = ["npcs/Mira", "npcs/Bob", "lore/Ring", "journal/Day 1"];
        let text = "---\ntype: journal\nwith: '[[Mira]]'\nseen:\n- \"[[npcs/Mira|M]]\"\n---\n\nMet [[mira|her]] and ![[Mira#Face]], not [[Bob]].\n`[[Mira]]` stays.\n";

        let (text, changed) = rename_in(
            root,
            &notes,
            "npcs/Mira",
            "npcs/Mira Vell",
            text,
            "journal/Day 1",
        )
        .unwrap();
        assert_eq!(changed, 4);
        assert_eq!(
            text,
            "---\ntype: journal\nwith: '[[Mira Vell]]'\nseen:\n- \"[[npcs/Mira Vell|M]]\"\n---\n\nMet [[Mira Vell|her]] and ![[Mira Vell#Face]], not [[Bob]].\n`[[Mira]]` stays.\n"
        );

        // Nothing to change.
        assert_eq!(
            rename_in(
                root,
                &notes,
                "npcs/Mira",
                "npcs/Ann",
                "[[Bob]]",
                "lore/Ring"
            ),
            None
        );
    }

    #[test]
    fn moving_a_folder_rewrites_links_to_every_note_in_it() {
        let root = Path::new("/c");
        let before = campaign(root, &["npcs/Mira", "npcs/Bob", "items/Bob", "Day 1"]);
        let after = campaign(root, &["people/Mira", "people/Bob", "items/Bob", "Day 1"]);
        let moved = [
            (root.join("npcs/Mira.md"), root.join("people/Mira.md")),
            (root.join("npcs/Bob.md"), root.join("people/Bob.md")),
        ];
        let rename = Rename {
            moved: &moved,
            before: &before,
            after: &after,
        };

        // Names that stay unique are kept; paths that named the folder follow it.
        let (text, changed) = relink_note(
            "[[Mira]], [[npcs/Bob]] and [[items/Bob]]\n",
            &root.join("Day 1.md"),
            &rename,
        )
        .unwrap();
        assert_eq!(text, "[[Mira]], [[people/Bob]] and [[items/Bob]]\n");
        assert_eq!(changed, 1);
    }

    #[test]
    fn renaming_keeps_front_matter_valid() {
        let root = Path::new("/c");
        let notes = ["npcs/Mira", "items/Cat"];
        let text = "---\nowner: '[[Mira]]'\n---\nBody\n";

        let (text, _) = rename_in(
            root,
            &notes,
            "npcs/Mira",
            "npcs/Mira's Cat",
            text,
            "items/Cat",
        )
        .unwrap();
        let note = note::parse(&text).unwrap();
        // The ' would end the quoted string, so the front matter is written out again.
        assert_eq!(
            note.front.get("owner"),
            Some(&Value::from("[[Mira's Cat]]"))
        );
        assert_eq!(note.preamble, "Body");
    }

    #[test]
    fn links_in_front_matter_are_read_as_yaml() {
        let text = "---\nowner: '[[John''s Long Johns]]'\nfriend: \"[[The \\\"Lucky\\\" Inn]]\"\nplain: [[Ælf's Hall]]\nlist:\n  - 'Saw [[Mira''s Cat]]'\n---\n\nBody [[John's Long Johns]] and [[It''s]].\n";
        assert_eq!(
            super::targets(text),
            [
                "John's Long Johns",
                "The \"Lucky\" Inn",
                "Ælf's Hall",
                "Mira's Cat",
                "John's Long Johns",
                // Outside the front matter, '' is just two quotes.
                "It''s",
            ]
        );
    }

    #[test]
    fn renaming_notes_with_quotes_in_their_names() {
        let root = Path::new("/c");
        let notes = ["npcs/John's Long Johns", "items/Ring"];
        let text = "---\nowner: '[[John''s Long Johns]]'\nalso: \"[[John's Long Johns]]\"\nbare: [[John's Long Johns]]\n---\n\nSee [[John's Long Johns|him]].\n";

        let (new, count) = rename_in(
            root,
            &notes,
            "npcs/John's Long Johns",
            "npcs/Jo's \"Big\" Shop",
            text,
            "items/Ring",
        )
        .unwrap();
        assert_eq!(count, 4, "{new}");
        let note = note::parse(&new).unwrap();
        for key in ["owner", "also"] {
            assert_eq!(
                note.front.get(key),
                Some(&Value::from("[[Jo's \"Big\" Shop]]")),
                "{new}"
            );
        }
        // Still in place, quoted the way it was.
        assert!(new.contains("owner: '[[Jo''s \"Big\" Shop]]'"), "{new}");
        assert!(new.contains("See [[Jo's \"Big\" Shop|him]]."), "{new}");
        assert_eq!(
            note.front.get("bare"),
            Some(&Value::Sequence(vec![Value::Sequence(vec![Value::from(
                "Jo's \"Big\" Shop"
            )])]))
        );
    }

    #[test]
    fn links_find_notes_whose_names_were_made_safe() {
        let root = Path::new("/c");
        let index = campaign(root, &["Chapter 1 - The Inn", "The 'Lucky' Inn", "Дракон"]);
        let from = root.join("Day.md");
        assert!(index.resolve("Chapter 1: The Inn", &from).is_some());
        assert!(index.resolve("The \"Lucky\" Inn", &from).is_some());
        assert!(index.resolve("дракон", &from).is_some());
        assert!(index.names().contains("CHAPTER 1: the inn"));
    }

    #[test]
    fn completing_a_link_in_front_matter_escapes_it() {
        let mut content = Content::with_text("---\nowner: '[[Jo\n---\n");
        content.move_to(Cursor {
            position: Position {
                line: 1,
                column: 12,
            },
            selection: None,
        });
        complete(&mut content, "John's Long Johns");
        assert_eq!(content.text(), "---\nowner: '[[John''s Long Johns]]\n---\n");
        assert_eq!(under_cursor(&content).as_deref(), Some("John's Long Johns"));
    }

    #[test]
    fn renaming_keeps_other_links_pointing_where_they_did() {
        let root = Path::new("/c");
        // Renaming npcs/Mira to npcs/Bob: "[[Bob]]" in npcs/Ann meant lore/Bob, but a Bob
        // in the same folder now wins, so it is written as a path.
        let notes = ["npcs/Mira", "npcs/Ann", "lore/Bob"];
        let (text, changed) = rename_in(
            root,
            &notes,
            "npcs/Mira",
            "npcs/Bob",
            "[[Mira]] and [[Bob]]",
            "npcs/Ann",
        )
        .unwrap();
        assert_eq!(changed, 2);
        assert_eq!(text, "[[Bob]] and [[lore/Bob]]");
    }

    #[test]
    fn renamed_note_relinks_itself() {
        let root = Path::new("/c");
        let notes = ["npcs/Mira"];
        let (text, _) = rename_in(
            root,
            &notes,
            "npcs/Mira",
            "npcs/Mirabel",
            "I am [[Mira]].md",
            "npcs/Mirabel",
        )
        .unwrap();
        assert_eq!(text, "I am [[Mirabel]].md");

        let (text, _) = rename_in(
            root,
            &notes,
            "npcs/Mira",
            "npcs/Mirabel",
            "[[Mira.md]]",
            "npcs/Mirabel",
        )
        .unwrap();
        assert_eq!(text, "[[Mirabel.md]]");
    }

    #[test]
    fn finds_notes_spelled_almost_like_a_link() {
        let root = Path::new("/c");
        let index = campaign(root, &["npcs/Bob", "npcs/Bobbin", "Old Mill", "Ox"]);
        let from = root.join("items/Ring.md");

        let names = |target| -> Vec<String> {
            index
                .similar(target, &from, 3)
                .into_iter()
                .map(|(link, _)| link)
                .collect()
        };
        assert_eq!(names("Bobby"), ["Bob", "Bobbin"]);
        assert_eq!(names("old mil"), ["Old Mill"]);
        assert_eq!(names("npcs/Bobb"), ["Bob"]);
        // Short names must be close.
        assert!(names("Oxen").is_empty());
        assert!(names("Castle").is_empty());
    }

    #[test]
    fn unlinking_keeps_the_shown_text() {
        let text =
            "owner: '[[Old Mill]]'\nSee [[old mill#Wheel|the wheel]], ![[Old Mill]] and [[Mira]].";
        assert_eq!(
            unlink_all(text, "Old Mill").unwrap(),
            "owner: 'Old Mill'\nSee the wheel, Old Mill and [[Mira]]."
        );
        assert!(unlink_all(text, "Nobody").is_none());
    }

    #[test]
    fn retargeting_points_links_at_another_note() {
        let text = "---\nowner: '[[Bobby]]'\n---\n\nSee [[bobby#Shop|his shop]] and [[Mira]].\n";
        let (new, count) = retarget_note(text, "Bobby", "Bob's Cat").unwrap();
        assert_eq!(count, 2);
        assert!(
            new.contains("[[Bob's Cat#Shop|his shop]] and [[Mira]]"),
            "{new}"
        );
        let front: Value = serde_yaml_ng::from_str(new.split("---").nth(1).unwrap()).unwrap();
        assert_eq!(front["owner"], Value::String("[[Bob's Cat]]".into()));
        assert!(retarget_note(text, "Nobody", "Bob").is_none());
    }

    #[test]
    fn types_links_in_the_editor() {
        let mut content = Content::with_text("");
        for c in "Met [".chars() {
            perform(&mut content, Action::Edit(Edit::Insert(c)));
        }
        perform(&mut content, Action::Edit(Edit::Insert('[')));
        assert_eq!(content.text(), "Met [[]]");

        for c in "Mi".chars() {
            perform(&mut content, Action::Edit(Edit::Insert(c)));
        }
        assert_eq!(query(&content).as_deref(), Some("Mi"));

        complete(&mut content, "Mira");
        assert_eq!(content.text(), "Met [[Mira]]");
        assert_eq!(query(&content), None);
        assert_eq!(under_cursor(&content).as_deref(), Some("Mira"));

        // Inside a finished link, nothing is suggested.
        content.perform(Action::Move(Motion::Left));
        content.perform(Action::Move(Motion::Left));
        content.perform(Action::Move(Motion::Left));
        assert_eq!(query(&content), None);
        assert_eq!(under_cursor(&content).as_deref(), Some("Mira"));
        content.perform(Action::Move(Motion::End));

        perform(&mut content, Action::Edit(Edit::Insert(' ')));
        perform(&mut content, Action::Edit(Edit::Insert('[')));
        perform(&mut content, Action::Edit(Edit::Insert('[')));
        for c in "Bob]]".chars() {
            perform(&mut content, Action::Edit(Edit::Insert(c)));
        }
        assert_eq!(content.text(), "Met [[Mira]] [[Bob]]");

        insert(&mut content);
        assert_eq!(content.text(), "Met [[Mira]] [[Bob]][[]]");
        assert_eq!(query(&content).as_deref(), Some(""));
    }

    #[test]
    fn highlighter_dims_marks_and_colors_links() {
        use text::Highlighter as _;

        let link = Color::from_rgb(0.0, 0.0, 1.0);
        let mark = Color::from_rgb(0.5, 0.5, 0.5);
        let colors = Colors {
            link,
            missing: Color::from_rgb(1.0, 0.0, 0.0),
            mark,
            bold: false,
            names: Names(Arc::new(HashSet::from([normalize("Bree")]))),
        };
        let mut highlighter = Highlighter::new(&colors);
        let mut spans = |line: &'static str| {
            highlighter
                .highlight_line(line)
                .map(|(range, (color, _))| (&line[range], color == mark))
                .collect::<Vec<_>>()
        };

        // Front matter: only its fences are marks, and its links are colored.
        assert_eq!(spans("---"), [("---", true)]);
        assert_eq!(
            spans("home: \"[[Bree]]\""),
            [("[[", true), ("Bree", false), ("]]", true)]
        );
        assert_eq!(spans("---"), [("---", true)]);
        assert_eq!(
            spans("- **Met** [[Bree]]"),
            [
                ("-", true),
                ("**", true),
                ("**", true),
                ("[[", true),
                ("Bree", false),
                ("]]", true)
            ]
        );
        // Code is left alone, but for its fences.
        assert_eq!(spans("```rust"), [("```rust", true)]);
        assert!(spans("let **x** = [[Bree]];").is_empty());
        assert_eq!(spans("```"), [("```", true)]);
    }
}
