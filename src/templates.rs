//! Campaign templates and saved types, kept in the user's config folder so every campaign
//! can use them.
//!
//! A template is a campaign's structure: its folders and their `_type.yaml` files, and its
//! own walkthrough questions in `_walkthrough.yaml` if it has them, with no notes. It is shared as an ordinary `.zip` file, so it can be opened, made or changed
//! with any zip tool. A saved type is one folder's type on its own, as a `<Name>.type.yaml`
//! file.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, Timelike};

use crate::campaign;
use crate::types::{self, ContentType};
use crate::walkthrough;

/// What a saved type's file name ends with.
pub const TYPE_EXTENSION: &str = ".type.yaml";

/// The biggest `_type.yaml` a template may hold, so a broken zip can't fill the memory.
const MAX_TYPE_SIZE: u64 = 1024 * 1024;
/// The most folders a template may hold.
const MAX_FOLDERS: usize = 5000;

/// Where imported and exported templates are kept: `templates` in the app's config folder.
pub fn templates_folder() -> Option<PathBuf> {
    crate::settings::config_folder().map(|folder| folder.join("templates"))
}

/// Where saved types are kept: `types` in the app's config folder.
pub fn types_folder() -> Option<PathBuf> {
    crate::settings::config_folder().map(|folder| folder.join("types"))
}

/// The folders of a campaign and their types, without any notes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Structure {
    /// Each folder's path in the campaign, like `items/weapons` (empty for the campaign
    /// folder itself), parents first, with its `_type.yaml` if it has one.
    pub folders: Vec<(String, Option<String>)>,
    /// The campaign's own walkthrough questions, from `_walkthrough.yaml` in the campaign
    /// folder. Without them, the walkthrough asks the built-in questions.
    pub walkthrough: Option<String>,
}

impl Structure {
    /// The starter folders of the default template, each with its type. They are plain
    /// folders, so a campaign also works as an Obsidian vault. They have no walkthrough
    /// file: the built-in questions are made for them, and get better as the app does.
    pub fn starter() -> Self {
        Structure {
            folders: types::STARTER_TYPES
                .iter()
                .map(|(folder, yaml)| (folder.to_string(), Some(yaml.to_string())))
                .collect(),
            walkthrough: None,
        }
    }

    /// Reads the folders, `_type.yaml` files and `_walkthrough.yaml` of the campaign at
    /// `root`. Hidden folders (like `.obsidian`) are left out, and so is every note.
    pub fn read(root: &Path) -> Self {
        fn walk(root: &Path, folder: &Path, folders: &mut Vec<(String, Option<String>)>) {
            let Ok(entries) = fs::read_dir(folder) else {
                return;
            };
            let mut children: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            children.sort_by_key(|path| path.to_string_lossy().to_lowercase());
            for child in children {
                let name = child
                    .strip_prefix(root)
                    .unwrap_or(&child)
                    .to_string_lossy()
                    .replace('\\', "/");
                folders.push((name, fs::read_to_string(child.join(types::TYPE_FILE)).ok()));
                walk(root, &child, folders);
            }
        }

        let mut folders = Vec::new();
        if let Ok(yaml) = fs::read_to_string(root.join(types::TYPE_FILE)) {
            folders.push((String::new(), Some(yaml)));
        }
        walk(root, root, &mut folders);
        Structure {
            folders,
            walkthrough: fs::read_to_string(root.join(walkthrough::FILE)).ok(),
        }
    }

    /// The folders in the campaign folder itself, like `items` and `npcs`.
    pub fn top_folders(&self) -> Vec<&str> {
        self.folders
            .iter()
            .map(|(folder, _)| folder.as_str())
            .filter(|folder| !folder.is_empty() && !folder.contains('/'))
            .collect()
    }

    /// How many folders have a type.
    pub fn typed(&self) -> usize {
        self.folders
            .iter()
            .filter(|(_, yaml)| yaml.is_some())
            .count()
    }

    /// Makes the folders inside `root`, each with its `_type.yaml`, and the walkthrough
    /// file. Folders that are there already are kept, and so are their types and a
    /// walkthrough file that is there.
    pub fn create_in(&self, root: &Path) -> Result<(), String> {
        for (folder, yaml) in &self.folders {
            let path = root.join(folder);
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            let file = path.join(types::TYPE_FILE);
            if let Some(yaml) = yaml
                && !file.exists()
            {
                fs::write(file, yaml).map_err(|error| error.to_string())?;
            }
        }
        let file = root.join(walkthrough::FILE);
        if let Some(yaml) = &self.walkthrough
            && !file.exists()
        {
            fs::write(file, yaml).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Writes the structure as a zip file: a folder entry for each folder, its
    /// `_type.yaml` inside it, and `_walkthrough.yaml` at the top.
    pub fn to_zip(&self) -> Result<Vec<u8>, String> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        // Dated now, rather than 1980, for whoever opens it in another zip tool.
        let now = chrono::Local::now().naive_local();
        if let Ok(time) = zip::DateTime::from_date_and_time(
            u16::try_from(now.year()).unwrap_or(1980),
            now.month() as u8,
            now.day() as u8,
            now.hour() as u8,
            now.minute() as u8,
            now.second() as u8,
        ) {
            options = options.last_modified_time(time);
        }
        let error = |error: zip::result::ZipError| error.to_string();

        for (folder, yaml) in &self.folders {
            if !folder.is_empty() {
                zip.add_directory(format!("{folder}/"), options)
                    .map_err(error)?;
            }
            if let Some(yaml) = yaml {
                let file = match folder.as_str() {
                    "" => types::TYPE_FILE.to_string(),
                    folder => format!("{folder}/{}", types::TYPE_FILE),
                };
                zip.start_file(file, options).map_err(error)?;
                zip.write_all(yaml.as_bytes())
                    .map_err(|error| error.to_string())?;
            }
        }
        if let Some(yaml) = &self.walkthrough {
            zip.start_file(walkthrough::FILE, options).map_err(error)?;
            zip.write_all(yaml.as_bytes())
                .map_err(|error| error.to_string())?;
        }

        Ok(zip.finish().map_err(error)?.into_inner())
    }

    /// Reads a template from the bytes of a zip file.
    ///
    /// Only folders, `_type.yaml` files and the top `_walkthrough.yaml` are read; anything
    /// else, like notes, is left out and counted. A zip of one folder, like Windows makes
    /// with "Send to > Compressed folder", is read as that folder's contents, unless the
    /// folder has a type of its own. Every type, and the walkthrough questions, must be
    /// ones the app can read.
    pub fn from_zip(bytes: &[u8]) -> Result<(Self, usize), String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|error| format!("it isn't a zip file the app can read ({error})"))?;

        let mut folders: Vec<String> = Vec::new();
        let mut files: Vec<(String, String)> = Vec::new();
        // The `_walkthrough.yaml` files, by the folder each is in; only the top one is used.
        let mut walkthroughs: Vec<(String, String)> = Vec::new();
        let mut skipped = 0;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
            let Some(path) = entry.enclosed_name() else {
                skipped += 1;
                continue;
            };
            let parts: Vec<String> = path
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            // macOS adds `__MACOSX` and `.DS_Store`; hidden folders aren't campaign folders.
            if parts
                .iter()
                .any(|part| part.starts_with('.') || part == "__MACOSX")
            {
                continue;
            }

            let name = parts.last().map(String::as_str);
            let depth = if entry.is_dir() {
                parts.len()
            } else if name == Some(types::TYPE_FILE) || name == Some(walkthrough::FILE) {
                if entry.size() > MAX_TYPE_SIZE {
                    return Err(format!("{} is too big", parts.join("/")));
                }
                let mut yaml = String::new();
                entry
                    .by_ref()
                    .take(MAX_TYPE_SIZE)
                    .read_to_string(&mut yaml)
                    .map_err(|error| format!("{}: {error}", parts.join("/")))?;
                let folder = parts[..parts.len() - 1].join("/");
                if name == Some(types::TYPE_FILE) {
                    files.push((folder, yaml));
                } else {
                    walkthroughs.push((folder, yaml));
                }
                parts.len() - 1
            } else {
                skipped += 1;
                continue;
            };
            // The folders a type is in are folders too, even without entries of their own.
            for end in 1..=depth {
                folders.push(parts[..end].join("/"));
            }
            if folders.len() > MAX_FOLDERS {
                return Err("it has too many folders for a template".into());
            }
        }

        // Parents first, then their subfolders, by name, like [`Structure::read`].
        folders.sort_by_key(|folder| folder.split('/').map(str::to_lowercase).collect::<Vec<_>>());
        folders.dedup();

        // One folder around everything, with no type of its own, is the zip's wrapper.
        let tops: Vec<&String> = folders.iter().filter(|f| !f.contains('/')).collect();
        if let [top] = tops[..]
            && files
                .iter()
                .all(|(folder, _)| !folder.is_empty() && folder != top)
            && walkthroughs.iter().all(|(folder, _)| !folder.is_empty())
            && folders.len() > 1
        {
            let top = top.clone();
            let prefix = format!("{top}/");
            folders = folders
                .iter()
                .filter_map(|folder| folder.strip_prefix(&prefix).map(String::from))
                .collect();
            for (folder, _) in files.iter_mut().chain(walkthroughs.iter_mut()) {
                *folder = match folder.strip_prefix(&prefix) {
                    Some(inside) => inside.to_string(),
                    None if *folder == top => String::new(),
                    None => folder.clone(),
                };
            }
        }

        // A walkthrough file in a subfolder isn't one the app reads, so it's left out.
        let (top, others): (Vec<_>, Vec<_>) = walkthroughs
            .into_iter()
            .partition(|(folder, _)| folder.is_empty());
        skipped += others.len();
        let mut structure = Structure {
            walkthrough: top.into_iter().next().map(|(_, yaml)| yaml),
            ..Structure::default()
        };
        if let Some((_, yaml)) = files.iter().find(|(folder, _)| folder.is_empty()) {
            structure.folders.push((String::new(), Some(yaml.clone())));
        }
        for folder in folders {
            for part in folder.split('/') {
                campaign::validate_name(part)
                    .map_err(|error| format!("the folder {folder}: {error}"))?;
            }
            let yaml = files
                .iter()
                .find(|(other, _)| *other == folder)
                .map(|(_, yaml)| yaml.clone());
            structure.folders.push((folder, yaml));
        }

        for (folder, yaml) in &structure.folders {
            if let Some(yaml) = yaml {
                types::parse(yaml).map_err(|error| {
                    let place = match folder.as_str() {
                        "" => types::TYPE_FILE.to_string(),
                        folder => format!("{folder}/{}", types::TYPE_FILE),
                    };
                    format!("{place} can't be read: {error}")
                })?;
            }
        }
        if let Some(yaml) = &structure.walkthrough {
            walkthrough::parse(yaml)
                .map_err(|error| format!("{} can't be read: {error}", walkthrough::FILE))?;
        }
        if structure.folders.is_empty() {
            return Err("it has no folders or types in it".into());
        }

        Ok((structure, skipped))
    }
}

/// A template a new campaign can start from.
#[derive(Debug, Clone, PartialEq)]
pub enum Template {
    /// No folders at all.
    Empty,
    /// The starter folders that come with the app.
    Starter,
    /// A template in the templates folder: its name, from its file name, and its file.
    Saved(String, PathBuf),
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Template::Empty => f.write_str("Empty: no folders"),
            Template::Starter => f.write_str("Starter folders (comes with the app)"),
            Template::Saved(name, _) => f.write_str(name),
        }
    }
}

impl Template {
    /// The folders and types the template makes.
    pub fn structure(&self) -> Result<Structure, String> {
        match self {
            Template::Empty => Ok(Structure::default()),
            Template::Starter => Ok(Structure::starter()),
            Template::Saved(name, path) => {
                let bytes = fs::read(path).map_err(|error| format!("{name}: {error}"))?;
                Structure::from_zip(&bytes)
                    .map(|(structure, _)| structure)
                    .map_err(|error| format!("{name}: {error}"))
            }
        }
    }
}

/// Every template a new campaign can start from: the starter folders, none, then the
/// templates in the templates folder, by name.
pub fn list() -> Vec<Template> {
    let mut saved = templates_folder()
        .map(|folder| list_in(&folder))
        .unwrap_or_default();
    let mut all = vec![Template::Starter, Template::Empty];
    all.append(&mut saved);
    all
}

fn list_in(folder: &Path) -> Vec<Template> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut templates: Vec<Template> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        })
        .map(|path| {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Template::Saved(name, path)
        })
        .collect();
    templates.sort_by_key(|template| template.to_string().to_lowercase());
    templates
}

/// The types saved in the types folder, by label.
pub fn saved_types() -> Vec<ContentType> {
    types_folder()
        .map(|folder| saved_types_in(&folder))
        .unwrap_or_default()
}

fn saved_types_in(folder: &Path) -> Vec<ContentType> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<ContentType> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| campaign::is_yaml(path))
        .filter_map(|path| types::parse(&fs::read_to_string(path).ok()?).ok())
        .collect();
    found.sort_by_key(|ty| ty.label().to_lowercase());
    found
}

/// A type as it is saved on its own: with every field it inherits, so it needs no other
/// type to work.
pub fn standalone(ty: &ContentType) -> Result<String, String> {
    ContentType {
        removed_fields: Vec::new(),
        ..ty.clone()
    }
    .to_yaml()
}

/// The file name a type is exported as, like `Weapon.type.yaml`.
pub fn type_file_name(ty: &ContentType) -> String {
    format!("{}{TYPE_EXTENSION}", campaign::note_file_name(&ty.label()))
}

/// What [`import`] added.
#[derive(Debug, Clone, PartialEq)]
pub enum Imported {
    /// A template, by name, and how many files in it (like notes) were left out.
    Template(String, usize),
    /// A saved type, by label.
    Type(String),
}

/// Adds the template (a `.zip`) or type (a `.yaml`) in `file` to the user's templates or
/// types, once it has been checked.
pub fn import(file: &Path) -> Result<Imported, String> {
    let (templates, types) = (
        templates_folder().ok_or("could not find the config folder")?,
        types_folder().ok_or("could not find the config folder")?,
    );
    import_into(file, &templates, &types)
}

fn import_into(file: &Path, templates: &Path, types_folder: &Path) -> Result<Imported, String> {
    let bytes = fs::read(file).map_err(|error| error.to_string())?;
    let file_name = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    if campaign::is_yaml(file) {
        let yaml = String::from_utf8(bytes).map_err(|_| "it isn't a text file".to_string())?;
        let ty = types::parse(&yaml).map_err(|error| format!("it isn't a type: {error}"))?;
        let name = type_file_name(&ty);
        let to = free_path(types_folder, &name, yaml.as_bytes())?;
        fs::write(&to, &yaml).map_err(|error| error.to_string())?;
        return Ok(Imported::Type(ty.label()));
    }

    let (_, skipped) = Structure::from_zip(&bytes)?;
    let to = free_path(templates, &file_name, &bytes)?;
    fs::write(&to, &bytes).map_err(|error| error.to_string())?;
    let name = to
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Imported::Template(name, skipped))
}

/// Where in `folder` to put a file called `name` with `bytes` in it: under that name, or
/// with a number if another file has it. A file with the same bytes is used as it is.
fn free_path(folder: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let (stem, extension) = match name.find('.') {
        Some(dot) => (&name[..dot], &name[dot..]),
        None => (name, ""),
    };
    for number in 1.. {
        let path = match number {
            1 => folder.join(name),
            n => folder.join(format!("{stem} {n}{extension}")),
        };
        match fs::read(&path) {
            Ok(existing) if existing == bytes => return Ok(path),
            Ok(_) => continue,
            Err(_) if path.exists() => continue,
            Err(_) => return Ok(path),
        }
    }
    unreachable!("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "dungeon_elements_templates_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_campaign_round_trips_through_a_zip_without_its_notes() {
        let root = temp_folder("round_trip");
        Structure::starter().create_in(&root).unwrap();
        fs::create_dir_all(root.join("items/weapons")).unwrap();
        fs::write(
            root.join("items/weapons/_type.yaml"),
            "type: weapon\nfields:\n  - key: damage\n    kind: dice\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("maps")).unwrap();
        fs::create_dir_all(root.join(".obsidian")).unwrap();
        fs::write(root.join("npcs/Mira.md"), "# Mira").unwrap();

        let structure = Structure::read(&root);
        assert!(structure.folders.iter().all(|(f, _)| !f.starts_with('.')));
        assert_eq!(structure.top_folders().len(), 10);
        let (back, skipped) = Structure::from_zip(&structure.to_zip().unwrap()).unwrap();
        assert_eq!(back, structure);
        assert_eq!(skipped, 0);

        let copy = temp_folder("round_trip_copy");
        back.create_in(&copy).unwrap();
        assert!(copy.join("maps").is_dir());
        assert!(!copy.join("npcs/Mira.md").exists());
        assert!(types::load(&copy.join("items/weapons")).unwrap().is_ok());
        assert_eq!(Structure::read(&copy), structure);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(copy).unwrap();
    }

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, text) in files {
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_zips_made_by_other_tools() {
        // Zipped from the file manager: one folder around it all, no folder entries,
        // notes and macOS extras.
        let bytes = zip_of(&[
            ("Cairn/npcs/_type.yaml", "type: npc\n"),
            ("Cairn/npcs/Mira.md", "# Mira"),
            ("Cairn/relics/", ""),
            ("__MACOSX/Cairn/._npcs", ""),
            ("Cairn/.DS_Store", ""),
        ]);
        let (structure, skipped) = Structure::from_zip(&bytes).unwrap();
        assert_eq!(
            structure.folders,
            [
                ("npcs".to_string(), Some("type: npc\n".to_string())),
                ("relics".to_string(), None),
            ]
        );
        assert_eq!(skipped, 1);

        // A single folder with a type of its own is the template, not a wrapper.
        let bytes = zip_of(&[("npcs/_type.yaml", "type: npc\n"), ("npcs/villains/", "")]);
        let (structure, _) = Structure::from_zip(&bytes).unwrap();
        assert_eq!(structure.top_folders(), ["npcs"]);
    }

    const QUESTIONS: &str = "notes:\n  hero: { folder: players, many: true }\nchapters:\n  - title: Heroes\n    questions:\n      - { ask: Who?, note: hero }\n";

    #[test]
    fn templates_carry_their_walkthrough_questions() {
        let root = temp_folder("walkthrough");
        Structure::starter().create_in(&root).unwrap();
        // The starter folders use the built-in questions.
        assert!(!root.join(walkthrough::FILE).exists());
        assert_eq!(Structure::read(&root).walkthrough, None);

        fs::write(root.join(walkthrough::FILE), QUESTIONS).unwrap();
        let structure = Structure::read(&root);
        assert_eq!(structure.walkthrough.as_deref(), Some(QUESTIONS));
        let (back, _) = Structure::from_zip(&structure.to_zip().unwrap()).unwrap();
        assert_eq!(back, structure);

        let copy = temp_folder("walkthrough_copy");
        back.create_in(&copy).unwrap();
        assert_eq!(
            fs::read_to_string(copy.join(walkthrough::FILE)).unwrap(),
            QUESTIONS
        );
        // A campaign's own questions are kept.
        fs::write(copy.join(walkthrough::FILE), "mine").unwrap();
        back.create_in(&copy).unwrap();
        assert_eq!(
            fs::read_to_string(copy.join(walkthrough::FILE)).unwrap(),
            "mine"
        );

        // In a zipped folder, the questions are in that folder; in a subfolder, they
        // aren't read.
        let bytes = zip_of(&[
            ("Cairn/players/_type.yaml", "type: player\n"),
            ("Cairn/_walkthrough.yaml", QUESTIONS),
            ("Cairn/players/_walkthrough.yaml", QUESTIONS),
        ]);
        let (structure, skipped) = Structure::from_zip(&bytes).unwrap();
        assert_eq!(structure.top_folders(), ["players"]);
        assert_eq!(structure.walkthrough.as_deref(), Some(QUESTIONS));
        assert_eq!(skipped, 1);

        let broken = zip_of(&[("players/", ""), ("_walkthrough.yaml", "chapters: [")]);
        let error = Structure::from_zip(&broken).unwrap_err();
        assert!(error.starts_with(walkthrough::FILE), "{error}");

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(copy).unwrap();
    }

    #[test]
    fn refuses_bad_templates() {
        assert!(Structure::from_zip(b"not a zip").is_err());
        assert!(Structure::from_zip(&zip_of(&[("Mira.md", "")])).is_err());
        let broken = zip_of(&[("npcs/_type.yaml", "fields: oops\n")]);
        let error = Structure::from_zip(&broken).unwrap_err();
        assert!(error.starts_with("npcs/_type.yaml"), "{error}");
        assert!(Structure::from_zip(&zip_of(&[("con/", "")])).is_err());
    }

    #[test]
    fn imports_templates_and_types() {
        let folder = temp_folder("import");
        let (templates, types_folder) = (folder.join("templates"), folder.join("types"));

        let zip = folder.join("Cairn.zip");
        fs::write(&zip, Structure::starter().to_zip().unwrap()).unwrap();
        let imported = import_into(&zip, &templates, &types_folder).unwrap();
        assert_eq!(imported, Imported::Template("Cairn".into(), 0));
        // The same file again is the same template; a different one gets a number.
        import_into(&zip, &templates, &types_folder).unwrap();
        fs::write(&zip, zip_of(&[("npcs/", "")])).unwrap();
        let imported = import_into(&zip, &templates, &types_folder).unwrap();
        assert_eq!(imported, Imported::Template("Cairn 2".into(), 0));
        let names: Vec<String> = list_in(&templates).iter().map(|t| t.to_string()).collect();
        assert_eq!(names, ["Cairn", "Cairn 2"]);
        assert_eq!(
            list_in(&templates)[0].structure().unwrap(),
            Structure::starter()
        );

        let ty = types::parse(types::TEST_ITEM).unwrap();
        let file = folder.join("whatever.yaml");
        fs::write(&file, standalone(&ty).unwrap()).unwrap();
        let imported = import_into(&file, &templates, &types_folder).unwrap();
        assert_eq!(imported, Imported::Type("Item".into()));
        assert!(types_folder.join("Item.type.yaml").is_file());
        assert_eq!(saved_types_in(&types_folder)[0].id, "item");

        fs::write(&file, "not: a type").unwrap();
        assert!(import_into(&file, &templates, &types_folder).is_err());

        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn saved_types_stand_alone() {
        let root = Path::new("campaign");
        let folders = [root.join("items"), root.join("items/weapons")];
        let mut own = types::Types::new();
        own.insert(folders[0].clone(), types::parse(types::TEST_ITEM));
        own.insert(
            folders[1].clone(),
            types::parse("type: weapon\nremoved_fields: [owner]\nfields:\n  - key: damage\n"),
        );
        let all = types::resolve(root, &folders, &own);
        let weapon = all[&folders[1]].as_ref().unwrap();

        let saved = types::parse(&standalone(weapon).unwrap()).unwrap();
        assert!(saved.removed_fields.is_empty());
        assert_eq!(saved.fields.len(), weapon.fields.len());
        assert_eq!(type_file_name(&saved), "Weapon.type.yaml");
    }
}
