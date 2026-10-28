//! The campaigns opened or made in the app, listed by Open Campaign so they don't have to
//! be found in the file browser. Saved as `campaigns.yaml` in the app's config folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub path: PathBuf,
    /// When it was last opened, as `2026-10-28 14:05` in the computer's time zone.
    pub opened: String,
}

impl Campaign {
    /// The campaign's name: its folder's.
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// When it was last opened, like "today", "yesterday", "5 days ago" or "2026-01-31".
    pub fn when(&self, today: chrono::NaiveDate) -> String {
        let Some(date) = self
            .opened
            .get(..10)
            .and_then(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        else {
            return self.opened.clone();
        };
        match (today - date).num_days() {
            0 => "today".into(),
            1 => "yesterday".into(),
            days @ 2..=13 => format!("{days} days ago"),
            _ => date.format("%Y-%m-%d").to_string(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    campaigns: Vec<Campaign>,
}

/// The campaigns, the one opened last first.
pub fn load() -> Vec<Campaign> {
    file().map(|path| load_from(&path)).unwrap_or_default()
}

fn load_from(path: &Path) -> Vec<Campaign> {
    let mut campaigns = fs::read_to_string(path)
        .ok()
        .and_then(|yaml| serde_yaml_ng::from_str::<File>(&yaml).ok())
        .unwrap_or_default()
        .campaigns;
    campaigns.sort_by(|a, b| b.opened.cmp(&a.opened));
    // A campaign listed twice (say, by hand) shows once, as last opened.
    let mut seen = std::collections::HashSet::new();
    campaigns.retain(|campaign| seen.insert(campaign.path.clone()));
    campaigns
}

/// Puts the campaign at `root` at the top of the list, as opened now.
pub fn remember(root: &Path) -> Result<(), String> {
    let path = file().ok_or("could not find the config folder")?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    remember_in(&path, root, now)
}

fn remember_in(path: &Path, root: &Path, now: String) -> Result<(), String> {
    let root = fs::canonicalize(root)
        .map(simplify)
        .unwrap_or_else(|_| root.to_path_buf());
    let mut campaigns = load_from(path);
    campaigns.retain(|campaign| campaign.path != root);
    campaigns.insert(
        0,
        Campaign {
            path: root,
            opened: now,
        },
    );
    save(path, campaigns)
}

/// Takes the campaign at `root` off the list. Its folder isn't touched.
pub fn forget(root: &Path) -> Result<(), String> {
    let path = file().ok_or("could not find the config folder")?;
    let mut campaigns = load_from(&path);
    campaigns.retain(|campaign| campaign.path != root);
    save(&path, campaigns)
}

fn save(path: &Path, campaigns: Vec<Campaign>) -> Result<(), String> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    let yaml = serde_yaml_ng::to_string(&File { campaigns }).map_err(|error| error.to_string())?;
    fs::write(path, yaml).map_err(|error| error.to_string())
}

/// Windows' full paths start with `\\?\`, which nobody wants to read.
fn simplify(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

fn file() -> Option<PathBuf> {
    crate::settings::config_folder().map(|folder| folder.join("campaigns.yaml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_campaigns_newest_first() {
        let folder =
            std::env::temp_dir().join(format!("dungeon_elements_recent_{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let file = folder.join("campaigns.yaml");
        let (a, b) = (folder.join("Curse of Strahd"), folder.join("Cairn"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();

        remember_in(&file, &a, "2026-10-01 10:00".into()).unwrap();
        remember_in(&file, &b, "2026-10-20 10:00".into()).unwrap();
        let names: Vec<String> = load_from(&file).iter().map(Campaign::name).collect();
        assert_eq!(names, ["Cairn", "Curse of Strahd"]);

        // Opening one again moves it to the top, once.
        remember_in(&file, &a, "2026-10-28 09:00".into()).unwrap();
        let campaigns = load_from(&file);
        assert_eq!(campaigns.len(), 2);
        assert_eq!(campaigns[0].name(), "Curse of Strahd");

        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
        assert_eq!(campaigns[0].when(today), "today");
        assert_eq!(campaigns[1].when(today), "8 days ago");
        assert_eq!(
            Campaign {
                path: a.clone(),
                opened: "2026-01-02 10:00".into()
            }
            .when(today),
            "2026-01-02"
        );

        fs::remove_dir_all(folder).unwrap();
    }
}
