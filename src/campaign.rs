//! Campaign folders on disk: creating them and listing what is inside.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

/// The optional starter folders for a new campaign.
///
/// They are plain folders, so a campaign also works as an Obsidian vault.
pub const TEMPLATE_FOLDERS: &[&str] = &["items", "locations", "lore", "npcs", "players", "quests"];

/// Creates a folder named `title` inside `parent` and returns its path.
///
/// If `with_template` is true, also creates the [`TEMPLATE_FOLDERS`] inside it.
pub fn create(parent: &Path, title: &str, with_template: bool) -> Result<PathBuf, String> {
    let title = title.trim();
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
            fs::create_dir(root.join(folder)).map_err(|error| error.to_string())?;
        }
    }

    Ok(root)
}

/// Checks that `name` can be used as a folder name on both Windows and Linux.
pub fn validate_name(name: &str) -> Result<(), String> {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if name.is_empty() {
        return Err("Give the campaign a title".into());
    }

    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || r#"<>:"/\|?*"#.contains(*c))
    {
        return Err(format!("The title can't contain {c:?}"));
    }

    if name.ends_with('.') || name == ".." {
        return Err("The title can't end with a period".into());
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

/// A folder or Markdown file inside a campaign.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    /// The folder's contents, or `None` if this entry is a file.
    pub children: Option<Vec<Entry>>,
}

/// Lists the folders and Markdown files under `folder`, folders first, sorted by name.
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
            } else if is_markdown(&path) {
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
        }
        assert!(create(&parent, "Curse of the Lich", true).is_err());

        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn creates_empty_campaign() {
        let parent = temp_folder("empty");
        let root = create(&parent, "Solo Run", false).unwrap();

        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);

        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn scans_folders_first_and_only_markdown() {
        let root = temp_folder("scan");
        fs::create_dir(root.join("npcs")).unwrap();
        fs::create_dir(root.join(".obsidian")).unwrap();
        fs::write(root.join("npcs").join("Mira.md"), "").unwrap();
        fs::write(root.join("Aardvark.md"), "").unwrap();
        fs::write(root.join("map.png"), "").unwrap();

        let entries = scan(&root);
        let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();

        assert_eq!(names, ["npcs", "Aardvark.md"]);
        assert_eq!(entries[0].children.as_ref().unwrap()[0].name, "Mira.md");

        fs::remove_dir_all(root).unwrap();
    }
}
