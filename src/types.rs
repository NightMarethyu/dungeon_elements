//! Content types: the `_type.yaml` file in a folder lists the fields every note in it has.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The name of the file in a folder that defines its content type.
pub const TYPE_FILE: &str = "_type.yaml";

/// The `_type.yaml` files of the default campaign template, by folder.
pub const STARTER_TYPES: &[(&str, &str)] = &[
    ("items", include_str!("../templates/default/items.yaml")),
    ("journal", include_str!("../templates/default/journal.yaml")),
    (
        "locations",
        include_str!("../templates/default/locations.yaml"),
    ),
    ("lore", include_str!("../templates/default/lore.yaml")),
    ("npcs", include_str!("../templates/default/npcs.yaml")),
    ("players", include_str!("../templates/default/players.yaml")),
    ("quests", include_str!("../templates/default/quests.yaml")),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentType {
    /// The id stored in each note's `type:` key.
    #[serde(rename = "type")]
    pub id: String,
    /// The name shown to the user, like "Item" in "New Item".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// An icon for the file tree. Not shown yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
}

impl ContentType {
    pub fn label(&self) -> String {
        self.label.clone().unwrap_or_else(|| title_case(&self.id))
    }

    /// The index of the field that names the note, which is also its filename.
    pub fn name_field(&self) -> Option<usize> {
        self.fields.iter().position(|field| field.key == "name")
    }

    /// Writes the type as the YAML of a `_type.yaml`, checking it the way [`parse`] does.
    pub fn to_yaml(&self) -> Result<String, String> {
        let yaml = serde_yaml_ng::to_string(self).map_err(|error| error.to_string())?;
        parse(&yaml)?;
        Ok(yaml)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The front matter key the value is stored under.
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub kind: Kind,
    /// The choices of a `choice` field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Width::is_half")]
    pub width: Width,
    /// For `link` fields: the folder (inside the campaign, like `locations`) whose notes
    /// are suggested first, and where a note the link names is made if there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl Field {
    /// A field that only one note has, added from the form. Its key is its label.
    pub fn for_note(label: &str, kind: Kind) -> Self {
        Field {
            key: label.to_string(),
            label: Some(label.to_string()),
            kind,
            options: Vec::new(),
            width: Width::Half,
            folder: None,
        }
    }

    pub fn label(&self) -> String {
        self.label.clone().unwrap_or_else(|| title_case(&self.key))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// One line of text.
    #[default]
    Text,
    Number,
    /// One of the field's `options`.
    Choice,
    /// A comma-separated list, stored as a YAML list (Obsidian reads `tags` as tags).
    Tags,
    /// The name of another note, stored as an Obsidian `[[link]]`.
    Link,
    Checkbox,
    /// Markdown text, stored as a `## Label` section in the note's body.
    Longtext,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Text,
        Kind::Longtext,
        Kind::Number,
        Kind::Choice,
        Kind::Tags,
        Kind::Link,
        Kind::Checkbox,
    ];

    /// The kinds a field added to a single note can have.
    ///
    /// A choice needs its options listed in the type, so it isn't one of them.
    pub const FOR_NOTE: [Kind; 6] = [
        Kind::Text,
        Kind::Longtext,
        Kind::Number,
        Kind::Tags,
        Kind::Link,
        Kind::Checkbox,
    ];
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Kind::Text => "Text (one line)",
            Kind::Number => "Number",
            Kind::Choice => "Choice",
            Kind::Tags => "Tags",
            Kind::Link => "Link to a note",
            Kind::Checkbox => "Checkbox",
            Kind::Longtext => "Long text",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Width {
    /// Half the form, so two fields sit side by side.
    #[default]
    Half,
    /// The whole width of the form.
    Full,
}

impl Width {
    pub const ALL: [Width; 2] = [Width::Half, Width::Full];

    fn is_half(&self) -> bool {
        *self == Width::Half
    }
}

impl std::fmt::Display for Width {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Width::Half => "Half width",
            Width::Full => "Full width",
        })
    }
}

pub fn parse(yaml: &str) -> Result<ContentType, String> {
    let ty: ContentType = serde_yaml_ng::from_str(yaml).map_err(|error| error.to_string())?;

    if ty.id.trim().is_empty() {
        return Err("the type has no name (`type:`)".into());
    }

    for (index, field) in ty.fields.iter().enumerate() {
        if field.key.trim().is_empty() {
            return Err(format!("field {} has no key", index + 1));
        }
        if field.key == "type" {
            return Err("`type` is reserved and can't be a field key".into());
        }
        if ty.fields[..index]
            .iter()
            .any(|other| other.key == field.key)
        {
            return Err(format!("the key {:?} is used twice", field.key));
        }
        if field.kind == Kind::Choice && field.options.is_empty() {
            return Err(format!("the choice field {:?} has no options", field.key));
        }
    }

    Ok(ty)
}

/// Reads the content type of `folder`: `None` if it has no `_type.yaml`.
pub fn load(folder: &Path) -> Option<Result<ContentType, String>> {
    let path = folder.join(TYPE_FILE);
    if !path.is_file() {
        return None;
    }

    Some(
        fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|yaml| parse(&yaml)),
    )
}

/// Turns a name like "Hit Points" into a key like `hit_points`.
pub fn key_from(name: &str) -> String {
    let mut key = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        } else if !key.ends_with('_') && !key.is_empty() {
            key.push('_');
        }
    }
    key.trim_end_matches('_').to_string()
}

/// Turns `snake_case` or `kebab-case` into "Title Case".
fn title_case(key: &str) -> String {
    key.split(['_', '-', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_types_are_valid() {
        for (folder, yaml) in STARTER_TYPES {
            let ty = parse(yaml).unwrap_or_else(|error| panic!("{folder}: {error}"));
            assert!(ty.name_field().is_some(), "{folder} has no name field");
        }
    }

    #[test]
    fn writes_what_it_reads() {
        for (folder, yaml) in STARTER_TYPES {
            let ty = parse(yaml).unwrap();
            let again = parse(&ty.to_yaml().unwrap()).unwrap();
            assert_eq!(format!("{again:?}"), format!("{ty:?}"), "{folder}");
        }

        let yaml = parse("type: x\nfields:\n  - key: a\n")
            .unwrap()
            .to_yaml()
            .unwrap();
        assert_eq!(yaml, "type: x\nfields:\n- key: a\n  kind: text\n");
    }

    #[test]
    fn makes_keys_from_names() {
        assert_eq!(key_from("Hit Points"), "hit_points");
        assert_eq!(key_from("  Armor Class (AC) "), "armor_class_ac");
        assert_eq!(key_from("Größe"), "größe");
    }

    #[test]
    fn fills_in_defaults() {
        let ty = parse("type: magic_item\nfields:\n  - key: attunement_required\n").unwrap();
        assert_eq!(ty.label(), "Magic Item");
        assert_eq!(ty.fields[0].label(), "Attunement Required");
        assert_eq!(ty.fields[0].kind, Kind::Text);
        assert_eq!(ty.fields[0].width, Width::Half);
    }

    #[test]
    fn rejects_bad_types() {
        assert!(parse("fields: []").is_err());
        assert!(parse("type: x\nfields:\n  - key: a\n  - key: a\n").is_err());
        assert!(parse("type: x\nfields:\n  - key: type\n").is_err());
        assert!(parse("type: x\nfields:\n  - key: a\n    kind: choice\n").is_err());
        assert!(parse("type: x\nfields:\n  - key: a\n    kind: colour\n").is_err());
        assert!(parse("type: x\nfeilds: []\n").is_err());
    }
}
