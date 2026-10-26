//! Internal links between notes, written the Obsidian way: `[[Note]]`, `[[Note|shown text]]`,
//! `[[Note#Heading]]`, `[[folder/Note]]`, and embeds like `![[Note]]`.
//!
//! A link names a note by its filename without `.md`, or by its path in the campaign when
//! two notes share a name. Renaming a note rewrites every link to it ([`relink_note`]), so
//! links keep working here and in Obsidian.

use std::collections::HashSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Color;
use iced::advanced::text;
use iced::widget::text_editor::{self, Action, Content, Cursor, Edit, Motion, Position};
use serde_yaml_ng::Value;

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

        if !line[i..].starts_with("[[") {
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
pub fn normalize(target: &str) -> String {
    let target = target.trim().replace('\\', "/");
    let target = target.trim_start_matches('/');
    let target = match target.len().checked_sub(3) {
        Some(cut) if target.is_char_boundary(cut) && target[cut..].eq_ignore_ascii_case(".md") => {
            &target[..cut]
        }
        _ => target,
    };
    target.to_lowercase()
}

/// The campaign's notes, for finding the note a link points to.
#[derive(Debug, Clone, Default)]
pub struct Index {
    notes: Vec<Entry>,
    /// Every name a link can use to reach a note, as [`normalize`]d: each note's
    /// path in the campaign and every shorter path ending in its filename.
    names: Names,
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
            notes,
            names: Names(Arc::new(names)),
        }
    }

    pub fn names(&self) -> Names {
        self.names.clone()
    }

    /// The note a link to `target` in the note at `from` points to, if there is one.
    ///
    /// Like Obsidian: names are compared without case; when several notes match, an exact
    /// path wins, then a note in the same folder as `from`, then the one nearest the top.
    pub fn resolve(&self, target: &str, from: &Path) -> Option<&Path> {
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

    /// Notes whose name contains `query`, best matches first: `(link text, label)` pairs.
    ///
    /// The label shows the folder when two notes share a name.
    pub fn suggest(&self, query: &str, from: &Path, limit: usize) -> Vec<(String, String)> {
        let query = normalize(query);
        let mut matches: Vec<&Entry> = self
            .notes
            .iter()
            .filter(|entry| entry.path != from && entry.key.contains(&query))
            .collect();

        let rank = |entry: &Entry| {
            let name = entry.name().to_lowercase();
            (
                !name.starts_with(&query),
                !name.contains(&query),
                name,
                entry.key.clone(),
            )
        };
        matches.sort_by_cached_key(|entry| rank(entry));

        matches
            .into_iter()
            .take(limit)
            .filter_map(|entry| {
                let link = self.link_text(&entry.path, from, false)?;
                let shared = self
                    .notes
                    .iter()
                    .filter(|other| other.name().eq_ignore_ascii_case(entry.name()))
                    .count()
                    > 1;
                let label = if shared {
                    entry.relative.clone()
                } else {
                    entry.name().to_string()
                };
                Some((link, label))
            })
            .collect()
    }
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

/// A note that was renamed: what changed, for rewriting the links to it.
pub struct Rename<'a> {
    pub old: &'a Path,
    pub new: &'a Path,
    /// The campaign's notes before and after the rename.
    pub before: &'a Index,
    pub after: &'a Index,
}

impl Rename<'_> {
    /// Where the note now at `path` was before the rename.
    fn was(&self, path: &Path) -> PathBuf {
        if path == self.new {
            self.old.to_path_buf()
        } else {
            path.to_path_buf()
        }
    }
}

/// Rewrites the links in `text`, the note at `from`, so they point where they did before
/// `rename`. Returns the new text and how many links changed.
///
/// Links to the renamed note get its new name. A link to another note is rewritten as a
/// path only if the rename would otherwise make it point to the renamed note instead.
/// Headings, shown text and `!` embeds are kept.
pub fn relink(text: &str, from: &Path, rename: &Rename) -> (String, usize) {
    let from_before = rename.was(from);
    let mut result = text.to_string();
    let mut changed = 0;

    // From the end, so earlier ranges stay valid.
    for link in find(text).into_iter().rev() {
        let target = &text[link.target.clone()];
        let Some(meant) = rename.before.resolve(target, &from_before) else {
            continue;
        };
        let meant = if meant == rename.old {
            rename.new
        } else {
            meant
        };
        if rename.after.resolve(target, from) == Some(meant) {
            continue;
        }

        let as_path = target.contains('/') || target.contains('\\');
        let Some(mut replacement) = rename.after.link_text(meant, from, as_path) else {
            continue;
        };
        if target.to_lowercase().ends_with(".md") {
            replacement.push_str(".md");
        }

        result.replace_range(link.target.clone(), &replacement);
        changed += 1;
    }

    (result, changed)
}

/// Rewrites the links in a whole note after `rename`, front matter included, the way
/// [`relink`] does. Returns `None` if no link changed.
///
/// Links in the front matter are YAML strings like `owner: '[[Mira]]'`. If a new name
/// can't be put in place as it is (say, `Mira's Cat` in single quotes), the front matter
/// is written out again so it still holds the right values.
pub fn relink_note(text: &str, from: &Path, rename: &Rename) -> Option<(String, usize)> {
    let (front, body) = note::split_front_matter(text).ok()?;
    let head = &text[..text.len() - body.len()];

    let (new_body, body_changes) = relink(body, from, rename);
    let Some(yaml) = front else {
        return (body_changes > 0).then(|| (format!("{head}{new_body}"), body_changes));
    };

    let (new_yaml, front_changes) = relink(yaml, from, rename);
    if front_changes + body_changes == 0 {
        return None;
    }
    if front_changes == 0 {
        return Some((format!("{head}{new_body}"), body_changes));
    }

    // Check that the front matter still reads back as the values it should have.
    let expected = serde_yaml_ng::from_str::<Value>(yaml)
        .ok()
        .map(|value| relink_value(value, from, rename));
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

/// Rewrites the links in the strings of a YAML value.
fn relink_value(value: Value, from: &Path, rename: &Rename) -> Value {
    match value {
        Value::String(text) => Value::String(relink(&text, from, rename).0),
        Value::Sequence(items) => Value::Sequence(
            items
                .into_iter()
                .map(|item| relink_value(item, from, rename))
                .collect(),
        ),
        Value::Mapping(mapping) => Value::Mapping(
            mapping
                .into_iter()
                .map(|(key, value)| (key, relink_value(value, from, rename)))
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
    links
        .into_iter()
        .find(|link| link.range.start <= position.column && position.column <= link.range.end)
        .map(|link| line[link.target].to_string())
}

/// Applies `action` to `content`, closing a typed `[[` with `]]` the way Obsidian does
/// (and typing over that `]]` instead of adding another).
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

/// How [`Highlighter`] colors links.
#[derive(Debug, Clone, PartialEq)]
pub struct Colors {
    /// Links to notes that exist.
    pub link: Color,
    /// Links to notes that don't exist yet.
    pub missing: Color,
    /// Whether links are bold too, for themes where their color is the text's.
    pub bold: bool,
    pub names: Names,
}

/// Colors the `[[links]]` in a text editor.
pub struct Highlighter {
    colors: Colors,
    /// Whether each line, up to the next one to color, starts inside a code block.
    in_code: Vec<bool>,
}

impl text::Highlighter for Highlighter {
    type Settings = Colors;
    /// The link's color, and whether it is bold.
    type Highlight = (Color, bool);
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, (Color, bool))>;

    fn new(colors: &Colors) -> Self {
        Highlighter {
            colors: colors.clone(),
            in_code: vec![false],
        }
    }

    fn update(&mut self, colors: &Colors) {
        self.colors = colors.clone();
        self.in_code.truncate(1);
    }

    fn change_line(&mut self, line: usize) {
        self.in_code.truncate(line + 1);
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let in_code = *self.in_code.last().unwrap_or(&false);
        let fence = is_fence(line);
        self.in_code.push(in_code != fence);

        let mut links = Vec::new();
        if !in_code && !fence {
            find_in_line(line, 0, &mut links);
        }

        links
            .into_iter()
            .map(|link| {
                let color = if self.colors.names.contains(&line[link.target]) {
                    self.colors.link
                } else {
                    self.colors.missing
                };
                (link.range, (color, self.colors.bold))
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.in_code.len() - 1
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

        let suggestions = index.suggest("mir", &from, 8);
        assert_eq!(
            suggestions,
            [
                ("Mira".into(), "Mira".into()),
                ("Mirage".into(), "Mirage".into()),
                ("Admiral".into(), "Admiral".into()),
            ]
        );
        let bobs = index.suggest("bob", &from, 8);
        assert_eq!(bobs[0].1, "items/Bob");
        assert_eq!(bobs[1].1, "npcs/Bob");
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
        let rename = Rename {
            old: &old_path,
            new: &new_path,
            before: &before,
            after: &after,
        };
        let from = root.join(format!("{from}.md"));
        relink_note(text, &from, &rename)
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
}
