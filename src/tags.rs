//! Tags: each tag a note has is also a note of its own in the tags folder, holding a list
//! of links to the notes tagged with it. The tagged notes keep their tags as a YAML list,
//! which Obsidian reads as tags, and the tag notes group them in any Markdown app.

use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

use crate::campaign;
use crate::links;
use crate::note;

/// A tag, with the folder (inside the campaign) its note is in: `(folder, tag)`.
pub type Tag = (String, String);

/// The tag note for `tag`, in `folder` of the campaign at `root`.
pub fn path(root: &Path, folder: &str, tag: &str) -> PathBuf {
    root.join(folder)
        .join(format!("{}.md", campaign::note_file_name(tag)))
}

/// The tags that have notes in `folder` of the campaign at `root`, by name.
pub fn known(root: &Path, folder: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
        return Vec::new();
    };
    let mut tags: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && campaign::is_markdown(path))
        .filter_map(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
        .collect();
    tags.sort_by_key(|tag| tag.to_lowercase());
    tags
}

/// The items of a list field's front matter value: a YAML list, or text separated by
/// commas (as tags typed in other apps sometimes are).
pub fn items(value: &Value) -> Vec<String> {
    let items = match value {
        Value::Sequence(items) => items.iter().map(note::value_to_text).collect(),
        Value::Null => Vec::new(),
        value => note::value_to_text(value)
            .split(',')
            .map(str::to_string)
            .collect::<Vec<_>>(),
    };
    items
        .into_iter()
        .map(|item| item.trim().trim_start_matches('#').trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// The tags in a note's `text`, as `(tag folder, tag)`, from the front matter keys of
/// `fields` (each with the folder its tag notes are in).
pub fn in_note(text: &str, fields: &[(String, String)]) -> Vec<(String, String)> {
    let Ok(note) = note::parse(text) else {
        return Vec::new();
    };
    let mut tags = Vec::new();
    for (key, folder) in fields {
        let Some(value) = note.front.get(key.as_str()) else {
            continue;
        };
        for tag in items(value) {
            let tag = (folder.clone(), tag);
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

/// Whether `line` is an item of a list that holds only a link `links_to` says is to the
/// tagged note.
fn lists(line: &str, links_to: &dyn Fn(&str) -> bool) -> bool {
    let trimmed = line.trim();
    let Some(item) = ["- ", "* ", "+ "]
        .iter()
        .find_map(|mark| trimmed.strip_prefix(mark))
    else {
        return false;
    };
    let found = links::find(item);
    let [link] = found.as_slice() else {
        return false;
    };
    link.range == (0..item.len()) && links_to(&item[link.target.clone()])
}

/// Whether a tag note's `text` lists a link `links_to` says is to the tagged note.
pub fn lists_note(text: &str, links_to: &dyn Fn(&str) -> bool) -> bool {
    text.lines().any(|line| lists(line, links_to))
}

/// How the tag notes in the campaign at `root` differ from the tags of a note that says
/// `text`, read from the front matter keys of `fields` (with their tag folders): the tags
/// whose notes don't list it, and the tag notes that list it though it doesn't have the
/// tag. `links_to` says whether a link in the tag note at the given path is to the note.
pub fn differences(
    root: &Path,
    text: &str,
    fields: &[(String, String)],
    links_to: &dyn Fn(&str, &Path) -> bool,
) -> (Vec<Tag>, Vec<Tag>) {
    let tags = in_note(text, fields);
    let missing = tags
        .iter()
        .filter(|(folder, tag)| {
            let file = path(root, folder, tag);
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            !lists_note(&text, &|target| links_to(target, &file))
        })
        .cloned()
        .collect();

    let mut folders: Vec<&String> = fields.iter().map(|(_, folder)| folder).collect();
    folders.dedup();
    let mut extra = Vec::new();
    for folder in folders {
        for tag in known(root, folder) {
            let file = path(root, folder, &tag);
            let has = tags
                .iter()
                .any(|(in_folder, other)| in_folder == folder && path(root, folder, other) == file);
            if has {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            if lists_note(&text, &|target| links_to(target, &file)) {
                extra.push((folder.clone(), tag));
            }
        }
    }
    (missing, extra)
}

/// A tag note's `text` with a link to the note `name` at the end of its list, or `None`
/// if it already lists a link `links_to` says is to that note.
pub fn add(text: &str, name: &str, links_to: &dyn Fn(&str) -> bool) -> Option<String> {
    if text.lines().any(|line| lists(line, links_to)) {
        return None;
    }
    let mut text = text.trim_end().to_string();
    // The link goes on the list at the end, or starts one after a blank line.
    let last = text.lines().next_back().unwrap_or_default().trim_start();
    if ["- ", "* ", "+ "].iter().any(|mark| last.starts_with(mark)) {
        text.push('\n');
    } else if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str(&format!("- [[{name}]]\n"));
    Some(text)
}

/// A tag note's `text` without the list items linking to the note `links_to` picks out,
/// or `None` if it has none.
pub fn remove(text: &str, links_to: &dyn Fn(&str) -> bool) -> Option<String> {
    let kept: Vec<&str> = text
        .split_inclusive('\n')
        .filter(|line| !lists(line, links_to))
        .collect();
    (kept.len() != text.split_inclusive('\n').count()).then(|| kept.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tags_from_lists_and_text() {
        let text = "---\ntags: [wizard, '#elf']\nkind: [a]\nmoods: grumpy, sleepy\n---\n";
        let fields = [
            ("tags".to_string(), "tags".to_string()),
            ("moods".to_string(), "moods".to_string()),
        ];
        assert_eq!(
            in_note(text, &fields),
            [
                ("tags".to_string(), "wizard".to_string()),
                ("tags".to_string(), "elf".to_string()),
                ("moods".to_string(), "grumpy".to_string()),
                ("moods".to_string(), "sleepy".to_string()),
            ]
        );
    }

    #[test]
    fn finds_tag_notes_out_of_date() {
        let root = std::env::temp_dir().join(format!("tags-test-{}", std::process::id()));
        let folder = root.join("tags");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("magic.md"), "- [[Ley Lines]]\n").unwrap();
        std::fs::write(folder.join("old.md"), "- [[Ley Lines]]\n").unwrap();
        std::fs::write(folder.join("other.md"), "- [[Mira]]\n").unwrap();

        // Obsidian took `old` off the note and gave it `new`.
        let text = "---\ntags: [magic, new]\n---\n";
        let fields = [("tags".to_string(), "tags".to_string())];
        let links_to = |target: &str, _: &Path| target == "Ley Lines";
        let (missing, extra) = differences(&root, text, &fields, &links_to);
        assert_eq!(missing, [("tags".to_string(), "new".to_string())]);
        assert_eq!(extra, [("tags".to_string(), "old".to_string())]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn adds_and_removes_links_to_a_note() {
        let gandalf = |target: &str| target.eq_ignore_ascii_case("gandalf");
        assert_eq!(
            add("", "Gandalf", &gandalf).as_deref(),
            Some("- [[Gandalf]]\n")
        );

        let text = "Wizards of the campaign.\n\n- [[Merlin]]\n";
        let added = add(text, "Gandalf", &gandalf).unwrap();
        assert_eq!(
            added,
            "Wizards of the campaign.\n\n- [[Merlin]]\n- [[Gandalf]]\n"
        );
        assert_eq!(add(&added, "Gandalf", &gandalf), None);
        assert_eq!(
            add("Wizards.\n", "Gandalf", &gandalf).as_deref(),
            Some("Wizards.\n\n- [[Gandalf]]\n")
        );

        // Only list items that are just the link go; other mentions stay.
        let text =
            "See [[Gandalf]].\n- [[Gandalf|The Grey]]\n- [[Merlin]]\n- [[Gandalf]] and [[Merlin]]";
        assert_eq!(
            remove(text, &gandalf).as_deref(),
            Some("See [[Gandalf]].\n- [[Merlin]]\n- [[Gandalf]] and [[Merlin]]")
        );
        assert_eq!(remove("- [[Merlin]]\n", &gandalf), None);
    }
}
