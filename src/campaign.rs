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

/// The folder in a campaign that pictures added from elsewhere are copied to.
pub const PICTURES_FOLDER: &str = "pictures";

/// Copies the picture `file` into the campaign `root`'s pictures folder and returns where
/// it went. A picture already in the campaign stays where it is, and one copied before
/// isn't copied again. If another picture has the name, the copy gets a number.
pub fn copy_picture(root: &Path, file: &Path) -> Result<PathBuf, String> {
    if file.starts_with(root) {
        return Ok(file.to_path_buf());
    }
    let bytes = fs::read(file).map_err(|error| error.to_string())?;
    let folder = root.join(PICTURES_FOLDER);
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;

    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let extension = file.extension().unwrap_or_default().to_string_lossy();
    for number in 1.. {
        let name = match number {
            1 => format!("{stem}.{extension}"),
            n => format!("{stem} {n}.{extension}"),
        };
        let path = folder.join(name);
        match fs::read(&path) {
            Ok(existing) if existing == bytes => return Ok(path),
            Ok(_) => continue,
            Err(_) if path.exists() => continue,
            Err(_) => {
                fs::write(&path, &bytes).map_err(|error| error.to_string())?;
                return Ok(path);
            }
        }
    }
    unreachable!("some number is free")
}

/// Checks that `name` can be used as a file or folder name on both Windows and Linux.
/// The file name, without `.md`, for a note called `name`.
///
/// A note's name can be anything, like `John's "Long" Johns` or `Chapter 1: The Inn`, but
/// its file name must work in Windows Explorer, macOS and Linux, and in a `[[link]]`. So
/// characters that can't be in one are swapped for something close (`:` for ` -`, `"` for
/// `'`, brackets for parentheses) or left out (`?`, `*`, `#`, `^`). Accents, other
/// alphabets and emoji are kept as they are.
pub fn note_file_name(name: &str) -> String {
    let safe = safe_name(name);
    if safe.is_empty() {
        "Untitled".into()
    } else {
        safe
    }
}

/// [`note_file_name`], but empty if nothing is left of `name`.
pub fn safe_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let mut swapped = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            ':' => swapped.push_str(" -"),
            '/' | '\\' | '|' => swapped.push('-'),
            '"' => swapped.push('\''),
            '<' | '[' => swapped.push('('),
            '>' | ']' => swapped.push(')'),
            '?' | '*' | '#' | '^' => {}
            c if c.is_control() => swapped.push(' '),
            c => swapped.push(c),
        }
    }

    // Runs of spaces become one, and Windows drops spaces and periods at the ends.
    let mut safe = swapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = safe.trim_matches(|c: char| c == '.' || c == ' ').len();
    if trimmed != safe.len() {
        safe = safe
            .trim_matches(|c: char| c == '.' || c == ' ')
            .to_string();
    }

    let stem = safe.split('.').next().unwrap_or(&safe);
    if RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        safe.push('_');
    }
    safe
}

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

    #[test]
    fn copies_pictures_into_the_campaign() {
        let root = temp_folder("pictures");
        let outside = temp_folder("pictures_outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let map = outside.join("map.png");
        fs::write(&map, b"one").unwrap();

        let copy = copy_picture(&root, &map).unwrap();
        assert_eq!(copy, root.join("pictures/map.png"));
        // The same picture again isn't copied twice.
        assert_eq!(copy_picture(&root, &map).unwrap(), copy);
        // A different one with the same name gets a number.
        fs::write(&map, b"two").unwrap();
        assert_eq!(
            copy_picture(&root, &map).unwrap(),
            root.join("pictures/map 2.png")
        );
        // One in the campaign stays put.
        assert_eq!(copy_picture(&root, &copy).unwrap(), copy);

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

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
    fn note_names_become_safe_file_names() {
        let cases = [
            ("John's Long Johns", "John's Long Johns"),
            ("The \"Lucky\" Inn", "The 'Lucky' Inn"),
            ("Chapter 1: The Inn", "Chapter 1 - The Inn"),
            ("What?", "What"),
            ("Room #3 [old]", "Room 3 (old)"),
            ("A/B\\C|D", "A-B-C-D"),
            ("Ends with dots...", "Ends with dots"),
            ("  spaced   out  ", "spaced out"),
            ("Con", "Con_"),
            ("Ælfrida Ō'Brien 🐉", "Ælfrida Ō'Brien 🐉"),
            ("Дракон", "Дракон"),
            ("???", "Untitled"),
        ];
        for (name, file) in cases {
            assert_eq!(note_file_name(name), file, "{name}");
            assert!(validate_name(&note_file_name(name)).is_ok(), "{name}");
            // Doing it twice changes nothing.
            assert_eq!(note_file_name(file), file, "{file}");
        }
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
