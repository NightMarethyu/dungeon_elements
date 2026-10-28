//! Content types: the `_type.yaml` file in a folder lists the fields every note in it has.
//!
//! A subfolder's type is a kind of its parent folder's, like a child class: Weapons in
//! Items has every Item field, and the fields its own `_type.yaml` adds or changes. A
//! subfolder with no `_type.yaml` has its parent's type.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The name of the file in a folder that defines its content type.
pub const TYPE_FILE: &str = "_type.yaml";

/// Each folder's content type, or what is wrong with its `_type.yaml`.
pub type Types = HashMap<PathBuf, Result<ContentType, String>>;

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
    /// The keys of the parent folder's fields that this subfolder's notes don't have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_fields: Vec<String>,
    /// The ids of the types this one is a kind of, nearest first. Worked out by
    /// [`resolve`], never saved.
    #[serde(skip)]
    pub parents: Vec<String>,
}

impl ContentType {
    pub fn label(&self) -> String {
        self.label.clone().unwrap_or_else(|| title_case(&self.id))
    }

    /// Whether this is the type `id`, or a kind of it.
    pub fn is_a(&self, id: &str) -> bool {
        self.id == id || self.parents.iter().any(|parent| parent == id)
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
    /// For `link` and `steps` fields: the folder (inside the campaign, like `locations`) whose notes
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
    /// An ordered checklist, stored as a `## Label` section of `- [ ]` task items. A step
    /// can be a `[[link]]` to another note, so a quest can be made of smaller quests.
    Steps,
    /// Dice to roll, like `2d6+3` or `1d20+5 advantage`: picked as a number of dice, a die,
    /// a modifier and a special rule, and stored as that text.
    Dice,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::Text,
        Kind::Longtext,
        Kind::Steps,
        Kind::Number,
        Kind::Dice,
        Kind::Choice,
        Kind::Tags,
        Kind::Link,
        Kind::Checkbox,
    ];

    /// The kinds a field added to a single note can have.
    ///
    /// A choice needs its options listed in the type, so it isn't one of them.
    pub const FOR_NOTE: [Kind; 8] = [
        Kind::Text,
        Kind::Longtext,
        Kind::Steps,
        Kind::Number,
        Kind::Dice,
        Kind::Tags,
        Kind::Link,
        Kind::Checkbox,
    ];

    /// Whether fields of this kind are `## Label` sections of the body, not front matter.
    pub fn is_section(self) -> bool {
        matches!(self, Kind::Longtext | Kind::Steps)
    }

    /// Whether fields of this kind can link to notes, and so have a folder they link to.
    pub fn links(self) -> bool {
        matches!(self, Kind::Link | Kind::Steps)
    }
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
            Kind::Steps => "Steps",
            Kind::Dice => "Dice",
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

/// The type of a subfolder whose own `_type.yaml` is `own`, in a folder of type `parent`:
/// the parent's fields in the parent's order (less the ones `own` removes, and as `own`
/// changes them), then the fields only `own` has.
pub fn inherit(parent: &ContentType, own: &ContentType) -> ContentType {
    // The name is the note's filename, so it can't be removed.
    let kept: Vec<&Field> = parent
        .fields
        .iter()
        .filter(|field| field.key == "name" || !own.removed_fields.contains(&field.key))
        .collect();
    let mut fields: Vec<Field> = kept
        .iter()
        .map(|field| {
            own.fields
                .iter()
                .find(|changed| changed.key == field.key)
                .unwrap_or(field)
                .clone()
        })
        .collect();
    fields.extend(
        own.fields
            .iter()
            .filter(|field| kept.iter().all(|kept| kept.key != field.key))
            .cloned(),
    );

    let mut parents = vec![parent.id.clone()];
    parents.extend(parent.parents.iter().cloned());
    ContentType {
        fields,
        parents,
        ..own.clone()
    }
}

/// Works out the type of every folder in `folders` (listed parents first) from their own
/// `_type.yaml` files, `own`: a subfolder's type is a kind of its parent's (see
/// [`inherit`]), and a subfolder with no `_type.yaml` has its parent's type. Folders in
/// the campaign folder itself don't take the campaign folder's type.
pub fn resolve(root: &Path, folders: &[PathBuf], own: &Types) -> Types {
    let mut all = Types::new();
    if let Some(ty) = own.get(root) {
        all.insert(root.to_path_buf(), ty.clone());
    }

    for folder in folders {
        let parent = folder
            .parent()
            .filter(|parent| *parent != root)
            .and_then(|parent| all.get(parent));
        let ty = match (own.get(folder), parent) {
            (Some(Ok(ty)), Some(Ok(parent))) => Ok(inherit(parent, ty)),
            (Some(Ok(_)), Some(Err(_))) => {
                Err("the type of the folder it is in can't be read, so it can't add to it".into())
            }
            (Some(ty), _) => ty.clone(),
            (None, Some(Ok(parent))) => Ok(parent.clone()),
            (None, _) => continue,
        };
        all.insert(folder.clone(), ty);
    }
    all
}

/// The type a note in a folder of type `folder` is read with when its `type:` is `id`:
/// the folder's, if it is that type or a kind of it (so an Item moved into Weapons is
/// read as a Weapon), or else the type in `types` with that id. `None` if there is none.
pub fn named<'a>(
    types: &'a Types,
    folder: Option<&'a ContentType>,
    id: &str,
) -> Option<&'a ContentType> {
    if let Some(ty) = folder.filter(|ty| ty.is_a(id)) {
        return Some(ty);
    }
    // A type is found in the folder that has it first, so `item` is Items' type, not
    // one copied to a subfolder.
    let mut found: Vec<_> = types
        .iter()
        .filter_map(|(folder, ty)| Some((folder, ty.as_ref().ok()?)))
        .filter(|(_, ty)| ty.id == id)
        .collect();
    found.sort_by(|a, b| a.0.cmp(b.0));
    found.first().map(|(_, ty)| *ty)
}

/// A name for one note of a folder: "Weapons" makes "Weapon", "Enemies" makes "Enemy".
pub fn singular(folder: &str) -> String {
    let name = title_case(folder);
    let lower = name.to_lowercase();
    let cut = if lower.ends_with("ies") && name.len() > 4 {
        return format!("{}y", &name[..name.len() - 3]);
    } else if ["sses", "xes", "ches", "shes"]
        .iter()
        .any(|end| lower.ends_with(end))
    {
        2
    } else if lower.ends_with('s') && !lower.ends_with("ss") && name.len() > 3 {
        1
    } else {
        0
    };
    name[..name.len() - cut].to_string()
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
pub fn title_case(key: &str) -> String {
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

    fn folders(root: &Path, names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|name| root.join(name)).collect()
    }

    #[test]
    fn subfolders_are_kinds_of_their_parents() {
        let root = Path::new("campaign");
        let folders = folders(
            root,
            &[
                "items",
                "items/weapons",
                "items/weapons/swords",
                "items/gear",
                "npcs",
            ],
        );
        let mut own = Types::new();
        own.insert(root.join("items"), parse(STARTER_TYPES[0].1));
        own.insert(
            root.join("items/weapons"),
            parse(
                "type: weapon\nlabel: Weapon\nremoved_fields: [owner, name]\nfields:\n  - key: damage\n  - key: rarity\n    kind: text\n",
            ),
        );
        own.insert(
            root.join("items/weapons/swords"),
            parse("type: sword\nfields:\n  - key: hilt\n"),
        );

        let all = resolve(root, &folders, &own);
        let keys = |folder: &str| -> Vec<String> {
            let ty = all[&root.join(folder)].as_ref().unwrap();
            ty.fields.iter().map(|field| field.key.clone()).collect()
        };
        assert_eq!(
            keys("items"),
            ["name", "value", "rarity", "owner", "history", "stats"]
        );
        // Owner is removed (the name can't be), rarity changed in place, damage added.
        assert_eq!(
            keys("items/weapons"),
            ["name", "value", "rarity", "history", "stats", "damage"]
        );
        assert_eq!(
            keys("items/weapons/swords"),
            [&keys("items/weapons")[..], &["hilt".to_string()]].concat()
        );
        // Gear has no type of its own, so it has Items'.
        assert_eq!(keys("items/gear"), keys("items"));
        assert!(!all.contains_key(&root.join("npcs")));

        let weapon = all[&root.join("items/weapons")].as_ref().unwrap();
        assert_eq!(weapon.fields[2].kind, Kind::Text);
        let sword = all[&root.join("items/weapons/swords")].as_ref().unwrap();
        assert_eq!(sword.parents, ["weapon", "item"]);
        assert!(sword.is_a("item") && sword.is_a("sword") && !sword.is_a("npc"));

        // A note that says it's an item is read as a sword in Swords, and as an item elsewhere.
        assert_eq!(named(&all, Some(sword), "item").unwrap().id, "sword");
        assert_eq!(named(&all, None, "item").unwrap().fields.len(), 6);
        assert_eq!(named(&all, None, "weapon").unwrap().id, "weapon");
        assert!(named(&all, None, "npc").is_none());

        // Nothing worked out is saved.
        assert!(!sword.to_yaml().unwrap().contains("weapon"));
    }

    #[test]
    fn subfolders_of_broken_types() {
        let root = Path::new("campaign");
        let folders = folders(root, &["a", "a/b", "a/c"]);
        let mut own = Types::new();
        own.insert(root.join("a"), Err("broken".into()));
        own.insert(root.join("a/b"), parse("type: b\n"));
        let all = resolve(root, &folders, &own);
        assert!(all[&root.join("a/b")].is_err());
        assert!(!all.contains_key(&root.join("a/c")));
    }

    #[test]
    fn names_one_note_of_a_folder() {
        assert_eq!(singular("weapons"), "Weapon");
        assert_eq!(singular("Enemies"), "Enemy");
        assert_eq!(singular("armor"), "Armor");
        assert_eq!(singular("bosses"), "Boss");
        assert_eq!(singular("magic_items"), "Magic Item");
        assert_eq!(singular("gas"), "Gas");
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
