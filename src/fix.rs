//! Fixes for some of the problems [`crate::check`] finds, made to a note's text.

use serde_yaml_ng::Value;

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
