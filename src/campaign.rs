//! Campaign folders on disk: creating them and listing what is inside.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::types;

/// The optional starter folders for a new campaign.
///
/// Each gets a `_type.yaml` from the default template. They are plain folders,
/// so a campaign also works as an Obsidian vault.
pub const TEMPLATE_FOLDERS: &[&str] = &[
    "items",
    "journal",
    "locations",
    "lore",
    "npcs",
    "players",
    "quests",
];

/// Creates a folder named `title` inside `parent` and returns its path.
///
/// If `with_template` is true, also creates the [`TEMPLATE_FOLDERS`] inside it, each with its content type.
pub fn create(parent: &Path, title: &str, with_template: bool) -> Result<PathBuf, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Give the campaign a title".into());
    }
    validate_name(title)?;

    if !parent.is_dir() {
        return Err(format!("{} is not a folder", parent.display()));
    }

    let root = parent.join(title);
    if root.exists() {
        return Err(format!("{} already exists", root.display()));
    }

    fs::create_dir(&root).map_err(|error| error.to_string())?;

    if with_template {
        for folder in TEMPLATE_FOLDERS {
            let path = root.join(folder);
            fs::create_dir(&path).map_err(|error| error.to_string())?;

            if let Some((_, yaml)) = types::STARTER_TYPES.iter().find(|(name, _)| name == folder) {
                fs::write(path.join(types::TYPE_FILE), yaml).map_err(|error| error.to_string())?;
            }
        }
    }

    Ok(root)
}

/// Creates a folder named `name` inside `root` and returns its path.
///
/// If `type_yaml` is given, it is written to the folder's `_type.yaml`.
pub fn create_folder(root: &Path, name: &str, type_yaml: Option<&str>) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the folder a name".into());
    }
    validate_name(name)?;

    let path = root.join(name);
    if path.exists() {
        return Err(format!("{name} already exists"));
    }

    fs::create_dir(&path).map_err(|error| error.to_string())?;
    if let Some(yaml) = type_yaml {
        fs::write(path.join(types::TYPE_FILE), yaml).map_err(|error| error.to_string())?;
    }

    Ok(path)
}

/// Checks that `name` can be used as a file or folder name on both Windows and Linux.
pub fn validate_name(name: &str) -> Result<(), String> {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if name.is_empty() {
        return Err("The name can't be empty".into());
    }

    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || r#"<>:"/\|?*"#.contains(*c))
    {
        return Err(format!("The name can't contain {c:?}"));
    }

    if name.ends_with('.') || name == ".." {
        return Err("The name can't end with a period".into());
    }

    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(format!("{name} is a reserved name on Windows"));
    }

    Ok(())
}

/// A folder, Markdown file, or YAML file inside a campaign.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    /// The folder's contents, or `None` if this entry is a file.
    pub children: Option<Vec<Entry>>,
}

/// Lists the folders, Markdown files, and YAML files under `folder`, folders first, sorted by name.
///
/// Hidden entries (such as `.obsidian` or `.git`) are skipped.
pub fn scan(folder: &Path) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut entries: Vec<Entry> = read_dir
        .flatten()
        .filter_map(|dir_entry| {
            let path = dir_entry.path();
            let name = dir_entry.file_name().to_string_lossy().into_owned();

            if name.starts_with('.') {
                return None;
            }

            if path.is_dir() {
                let children = Some(scan(&path));
                Some(Entry {
                    path,
                    name,
                    children,
                })
            } else if is_markdown(&path) || is_yaml(&path) {
                Some(Entry {
                    path,
                    name,
                    children: None,
                })
            } else {
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| match (a.children.is_some(), b.children.is_some()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    entries
}

/// Whether `path` has a `.md` extension.
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// Whether `path` has a `.yaml` or `.yml` extension, like a folder's `_type.yaml`.
pub fn is_yaml(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("dungeon_elements_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("what?").is_err());
        assert!(validate_name("ends.").is_err());
        assert!(validate_name("con").is_err());
        assert!(validate_name("NUL.txt").is_err());
        assert!(validate_name("The Sunken Keep").is_ok());
        assert!(validate_name("Console Wars").is_ok());
    }

    #[test]
    fn creates_campaign_with_template() {
        let parent = temp_folder("template");
        let root = create(&parent, "  Curse of the Lich  ", true).unwrap();

        assert_eq!(root, parent.join("Curse of the Lich"));
        for folder in TEMPLATE_FOLDERS {
            assert!(root.join(folder).is_dir());
            let ty = types::load(&root.join(folder)).expect("has a type");
            assert!(ty.is_ok(), "{folder}: {ty:?}");
        }
        assert!(create(&parent, "Curse of the Lich", true).is_err());

        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn creates_folders() {
        let root = temp_folder("folders");

        let factions = create_folder(&root, " factions ", Some("type: faction\n")).unwrap();
        assert_eq!(factions, root.join("factions"));
        assert!(types::load(&factions).unwrap().is_ok());

        let notes = create_folder(&root, "notes", None).unwrap();
        assert!(types::load(&notes).is_none());

        assert!(create_folder(&root, "factions", None).is_err());
        assert!(create_folder(&root, "", None).is_err());
        assert!(create_folder(&root, "a/b", None).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn creates_empty_campaign() {
        let parent = temp_folder("empty");
        let root = create(&parent, "Solo Run", false).unwrap();

        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);

        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn scans_folders_first_and_only_markdown_and_yaml() {
        let root = temp_folder("scan");
        fs::create_dir(root.join("npcs")).unwrap();
        fs::create_dir(root.join(".obsidian")).unwrap();
        fs::write(root.join("npcs").join("Mira.md"), "").unwrap();
        fs::write(root.join("Aardvark.md"), "").unwrap();
        fs::write(root.join("map.png"), "").unwrap();
        fs::write(root.join("_type.yaml"), "").unwrap();

        let entries = scan(&root);
        let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();

        assert_eq!(names, ["npcs", "_type.yaml", "Aardvark.md"]);
        assert_eq!(entries[0].children.as_ref().unwrap()[0].name, "Mira.md");

        fs::remove_dir_all(root).unwrap();
    }
}
