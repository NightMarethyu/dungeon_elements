//! Fixes for some of the problems [`crate::check`] finds, made to a note's text.

use serde_yaml_ng::Value;

use crate::form::ALIASES_KEY;
use crate::links;
use crate::note::{self, Section};

/// Moves long text field `key` out of the front matter into a `## label` section, after
/// any text the section already has.
pub fn move_to_section(text: &str, key: &str, label: &str) -> Result<String, String> {
    let mut note = note::parse(text)?;
    let Some(value) = note.front.remove(key) else {
        return Ok(text.to_string());
    };
    let moved = note::value_to_text(&value);

    let existing = note.sections.iter_mut().find(|section| {
        section.heading.eq_ignore_ascii_case(label) || section.heading.eq_ignore_ascii_case(key)
    });
    match existing {
        Some(section) if section.body.trim().is_empty() => section.body = moved,
        Some(section) => section.body = format!("{}\n\n{moved}", section.body.trim_end()),
        None => note.sections.push(Section {
            heading: label.to_string(),
            body: moved,
            keep_empty: false,
        }),
    }

    note::to_markdown(&note.front, &note.sections, &note.preamble)
}

/// Sets front matter `key` to `value`, or removes it if `value` is `None`.
pub fn set_value(text: &str, key: &str, value: Option<&str>) -> Result<String, String> {
    let mut note = note::parse(text)?;
    match value {
        Some(value) => match note.front.get_mut(key) {
            Some(old) => *old = Value::String(value.to_string()),
            None => {
                note.front.insert(key.into(), value.into());
            }
        },
        None => {
            note.front.remove(key);
        }
    }
    note::to_markdown(&note.front, &note.sections, &note.preamble)
}

/// Adds `alias` to the note's `aliases`, the other names links can use, unless it is there.
pub fn add_alias(text: &str, alias: &str) -> Result<String, String> {
    let mut note = note::parse(text)?;
    let key = Value::from(ALIASES_KEY);
    let mut aliases = match note.front.get(&key) {
        Some(Value::Sequence(items)) => items.clone(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    };
    if aliases
        .iter()
        .any(|item| note::value_to_text(item).trim() == alias.trim())
    {
        return Ok(text.to_string());
    }
    aliases.push(alias.trim().into());
    note.front.insert(key, Value::Sequence(aliases));
    note::to_markdown(&note.front, &note.sections, &note.preamble)
}

/// Removes the alias that only spelled out the file name `old` (like `Chapter 1: The Inn`
/// for `Chapter 1 - The Inn`), unless it is `keep`. Aliases of their own, like Jerry, stay.
pub fn remove_alias_for(text: &str, old: &str, keep: &str) -> Result<String, String> {
    let mut note = note::parse(text)?;
    let key = Value::from(ALIASES_KEY);
    let Some(Value::Sequence(aliases)) = note.front.get_mut(&key) else {
        return Ok(text.to_string());
    };
    let before = aliases.len();
    let old = links::normalize(old);
    aliases.retain(|alias| {
        let alias = note::value_to_text(alias);
        alias.trim() == keep.trim() || links::normalize(&alias) != old
    });
    if aliases.len() == before {
        return Ok(text.to_string());
    }
    if aliases.is_empty() {
        note.front.remove(&key);
    }
    note::to_markdown(&note.front, &note.sections, &note.preamble)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_a_field_into_its_section() {
        let text = "---\nname: Ring\nhistory: Found in a well.\n---\n\nIntro.\n";
        assert_eq!(
            move_to_section(text, "history", "History").unwrap(),
            "---\nname: Ring\n---\n\nIntro.\n\n## History\n\nFound in a well.\n"
        );

        let text = "---\nhistory: More.\n---\n\n## History\n\nOld.\n";
        assert_eq!(
            move_to_section(text, "history", "History").unwrap(),
            "## History\n\nOld.\n\nMore.\n"
        );
    }

    #[test]
    fn adds_aliases_once() {
        let text = "---\nname: 'Chapter 1: The Inn'\n---\n";
        let added = add_alias(text, "Chapter 1: The Inn").unwrap();
        assert_eq!(
            added,
            "---\nname: 'Chapter 1: The Inn'\naliases:\n- 'Chapter 1: The Inn'\n---\n"
        );
        assert_eq!(add_alias(&added, "Chapter 1: The Inn").unwrap(), added);
        assert!(
            add_alias("---\naliases: Jerry\n---\n", "Boss")
                .unwrap()
                .contains("aliases:\n- Jerry\n- Boss\n")
        );
    }

    #[test]
    fn removes_the_alias_for_an_old_file_name() {
        let text =
            "---\nname: 'Chapter 2: Docks'\naliases:\n- Jerry\n- 'Chapter 1: The Inn'\n---\n";
        assert_eq!(
            remove_alias_for(text, "Chapter 1 - The Inn", "Chapter 2: Docks").unwrap(),
            "---\nname: 'Chapter 2: Docks'\naliases:\n- Jerry\n---\n"
        );
        assert_eq!(remove_alias_for(text, "Ring", "x").unwrap(), text);
    }

    #[test]
    fn sets_and_removes_values() {
        let text = "---\ntype: itme\nrarity: rare\n---\n\nText.\n";
        assert_eq!(
            set_value(text, "rarity", Some("Rare")).unwrap(),
            "---\ntype: itme\nrarity: Rare\n---\n\nText.\n"
        );
        assert_eq!(
            set_value(text, "type", None).unwrap(),
            "---\nrarity: rare\n---\n\nText.\n"
        );
    }
}
