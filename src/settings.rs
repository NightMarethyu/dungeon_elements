//! App-wide preferences, saved in the user's config folder.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The name of the chosen theme.
    pub theme: String,
    /// Whether the file tree and tabs show `.md` at the end of note names.
    pub show_extensions: bool,
    /// Whether the file tree shows `.yaml` files, such as each folder's `_type.yaml`.
    pub show_type_files: bool,
    /// Whether notes open in the reading view, formatted, rather than in an editor.
    pub open_in_reading: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: crate::theme::DEFAULT.into(),
            show_extensions: false,
            show_type_files: false,
            open_in_reading: false,
        }
    }
}

impl Settings {
    /// Reads the saved settings, or the defaults if there are none or they can't be read.
    pub fn load() -> Self {
        file()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|yaml| serde_yaml_ng::from_str(&yaml).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = file().ok_or("could not find the config folder")?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }

        let yaml = serde_yaml_ng::to_string(self).map_err(|error| error.to_string())?;
        fs::write(path, yaml).map_err(|error| error.to_string())
    }
}

fn file() -> Option<PathBuf> {
    config_folder().map(|folder| folder.join("settings.yaml"))
}

/// The app's config folder: `%APPDATA%\dungeon_elements` on Windows,
/// `$XDG_CONFIG_HOME/dungeon_elements` or `~/.config/dungeon_elements` elsewhere.
pub fn config_folder() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };

    base.map(|base| base.join("dungeon_elements"))
}
