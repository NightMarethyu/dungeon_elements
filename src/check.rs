//! Checks a campaign's files for formatting the app can't read as intended.
//!
//! Notes are plain Markdown, so they can be edited in Obsidian or any text editor.
//! When a campaign opens (and when it is refreshed), every note and `_type.yaml`
//! is checked, and anything that would show up wrong in a form is listed as a problem.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_yaml_ng::Value;

use crate::campaign::{self, Entry};
use crate::form::{self, REMOVED_KEY};
use crate::note;
use crate::types::{self, ContentType, Kind};

/// Something wrong with one file.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub path: PathBuf,
    pub message: String,
}

/// Checks every note and type file in the campaign.
pub fn campaign(
    root: &Path,
    tree: &[Entry],
    types: &HashMap<PathBuf, Result<ContentType, String>>,
) -> Vec<Problem> {
    let mut problems = Vec::new();

    let mut type_files: Vec<_> = types
        .iter()
        .filter_map(|(folder, ty)| ty.as_ref().err().map(|error| (folder, error)))
        .collect();
    type_files.sort();
    for (folder, error) in type_files {
        problems.push(Problem {
            path: folder.join(types::TYPE_FILE),
            message: error.clone(),
        });
    }

    let mut notes = Vec::new();
    collect_notes(tree, &mut notes);
    for path in notes {
        let Ok(text) = std::fs::read_to_string(&path) else {
            problems.push(Problem {
                message: "can't be read as text".into(),
                path,
            });
            continue;
        };
        let folder = path.parent().unwrap_or(root);
        for message in note(&text, folder, types) {
            problems.push(Problem {
                path: path.clone(),
                message,
            });
        }
    }

    problems
}

fn collect_notes(entries: &[Entry], notes: &mut Vec<PathBuf>) {
    for entry in entries {
        match &entry.children {
            Some(children) => collect_notes(children, notes),
            None if campaign::is_markdown(&entry.path) => notes.push(entry.path.clone()),
            None => {}
        }
    }
}

/// What is wrong with the note `text` in `folder`, read with the type it names or its folder's.
pub fn note(
    text: &str,
    folder: &Path,
    types: &HashMap<PathBuf, Result<ContentType, String>>,
) -> Vec<String> {
    let note = match note::parse(text) {
        Ok(note) => note,
        Err(error) => return vec![capitalize(&error)],
    };

    let mut problems = Vec::new();

    let named = match note.front.get("type") {
        None => None,
        Some(id) => {
            let id = note::value_to_text(id);
            let found = types.values().flatten().find(|ty| ty.id == id);
            if found.is_none() {
                problems.push(format!(
                    "Its `type: {id}` doesn't match any folder's type, so its folder's fields are used"
                ));
            }
            found
        }
    };
    let ty = named.or_else(|| types.get(folder).and_then(|ty| ty.as_ref().ok()));

    match note.front.get(REMOVED_KEY) {
        None | Some(Value::String(_)) => {}
        Some(Value::Sequence(keys)) if keys.iter().all(|key| matches!(key, Value::String(_))) => {}
        Some(_) => problems.push(format!(
            "`{REMOVED_KEY}` should be a list of field keys, so it is ignored"
        )),
    }

    if let Some(ty) = ty {
        for field in &ty.fields {
            let label = field.label();
            let value = note.front.get(field.key.as_str());

            if field.kind == Kind::Longtext {
                if value.is_some() {
                    problems.push(format!(
                        "{label} is in the front matter, but its text belongs in a `## {label}` section, so the form doesn't show it"
                    ));
                }
                let sections = note
                    .sections
                    .iter()
                    .filter(|section| {
                        section.heading.eq_ignore_ascii_case(&label)
                            || section.heading.eq_ignore_ascii_case(&field.key)
                    })
                    .count();
                if sections > 1 {
                    problems.push(format!(
                        "It has {sections} `## {label}` sections; the form shows the first as {label} and the others as separate fields"
                    ));
                }
                continue;
            }

            if let Some(value) = value
                && let Some(problem) = value_problem(field.kind, &field.options, value)
            {
                problems.push(format!("{label} {problem}"));
            }
        }
    }

    // Keys the form can't show as a field are kept in the file, but can only be edited as source.
    for (key, value) in &note.front {
        let key = note::value_to_text(key);
        let listed = form::is_reserved(&key)
            || ty.is_some_and(|ty| ty.fields.iter().any(|field| field.key == key));
        if !listed && form::kind_of(value).is_none() {
            problems.push(format!(
                "`{key}` is nested YAML, which the form can't show (it is kept as it is)"
            ));
        }
    }

    problems
}

/// What is wrong with `value` for a field of `kind`, as the end of a sentence.
fn value_problem(kind: Kind, options: &[String], value: &Value) -> Option<String> {
    let text = note::value_to_text(value);
    if matches!(value, Value::Null) || text.trim().is_empty() {
        return None;
    }

    match kind {
        Kind::Number => match value {
            Value::Number(_) => None,
            Value::String(text) if form::parse_number(text.trim()).is_some() => None,
            _ => Some(format!("should be a number, but is {text:?}")),
        },
        Kind::Choice if !options.iter().any(|option| option == &text) => Some(format!(
            "is {text:?}, which isn't one of its options ({})",
            options.join(", ")
        )),
        Kind::Checkbox => match value {
            Value::Bool(_) => None,
            _ if matches!(
                text.to_lowercase().as_str(),
                "true" | "false" | "yes" | "no"
            ) =>
            {
                None
            }
            _ => Some(format!(
                "should be true or false, but is {text:?} (the box shows unchecked)"
            )),
        },
        Kind::Tags if form::kind_of(value).is_none() || matches!(value, Value::Bool(_)) => {
            Some("should be a list of words".into())
        }
        Kind::Text | Kind::Link if matches!(value, Value::Mapping(_) | Value::Tagged(_)) => {
            Some("is nested YAML, but should be one line of text".into())
        }
        _ => None,
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// When the file at `path` was last changed, to tell whether it changed outside the app.
pub fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_types() -> HashMap<PathBuf, Result<ContentType, String>> {
        let mut item = types::parse(types::STARTER_TYPES[0].1).unwrap();
        let mut weight = types::Field::for_note("weight", Kind::Number);
        weight.label = Some("Weight".into());
        item.fields.push(weight);
        HashMap::from([(PathBuf::from("items"), Ok(item))])
    }

    fn check(text: &str) -> Vec<String> {
        note(text, Path::new("items"), &item_types())
    }

    #[test]
    fn accepts_good_notes() {
        assert!(check("---\ntype: item\nname: Ring\nvalue: 3\nrarity: Rare\nowner: '[[Bob]]'\n---\n\n## History\n\nOld.\n").is_empty());
        assert!(check("Just text, no front matter.\n").is_empty());
        assert!(
            check("---\nweight: '12.5'\nrarity: ''\nremoved_fields: [owner]\n---\n").is_empty()
        );
    }

    #[test]
    fn reports_broken_front_matter() {
        let problems = check("---\nname: Ring\n");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("closing"), "{problems:?}");
    }

    #[test]
    fn reports_values_of_the_wrong_kind() {
        let problems = check("---\nweight: lots\nrarity: Epic\n---\n");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("isn't one of its options"));
        assert!(problems[1].starts_with("Weight should be a number"));
    }

    #[test]
    fn reports_misplaced_and_repeated_sections() {
        let problems = check("---\nhistory: Old\n---\n\n## History\n\nA\n\n## history\n\nB\n");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("`## History` section"));
        assert!(problems[1].contains("2 `## History` sections"));
    }

    #[test]
    fn reports_unknown_types_and_nested_keys() {
        let problems = check("---\ntype: spell\nabilities:\n  str: 3\n---\n");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("`type: spell`"));
        assert!(problems[1].contains("`abilities` is nested YAML"));
    }

    #[test]
    fn checks_a_campaign_folder() {
        let root =
            std::env::temp_dir().join(format!("dungeon_elements_check_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("items")).unwrap();
        std::fs::create_dir_all(root.join("broken")).unwrap();
        std::fs::write(
            root.join("items").join(types::TYPE_FILE),
            types::STARTER_TYPES[0].1,
        )
        .unwrap();
        std::fs::write(root.join("broken").join(types::TYPE_FILE), "fields: [").unwrap();
        std::fs::write(
            root.join("items").join("Ring.md"),
            "---\nrarity: Epic\n---\n",
        )
        .unwrap();
        std::fs::write(root.join("items").join("Cup.md"), "---\nvalue: 2\n---\n").unwrap();

        let tree = campaign::scan(&root);
        let mut types = HashMap::new();
        for folder in ["items", "broken"] {
            let folder = root.join(folder);
            types.insert(folder.clone(), types::load(&folder).unwrap());
        }

        let problems = campaign(&root, &tree, &types);
        let paths: Vec<_> = problems
            .iter()
            .map(|problem| problem.path.clone())
            .collect();
        assert_eq!(
            paths,
            [
                root.join("broken").join(types::TYPE_FILE),
                root.join("items").join("Ring.md")
            ]
        );

        std::fs::remove_dir_all(&root).unwrap();
    }
}
