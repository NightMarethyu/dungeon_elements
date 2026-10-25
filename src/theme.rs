//! Color themes: YAML files that give a color to each part of the window.
//!
//! The built-in themes live in the `themes/` folder of the repository and are
//! compiled into the app. Users add their own by dropping YAML files with the
//! same layout into [`user_folder`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use iced::Color;
use serde::Deserialize;

const DARK: &str = include_str!("../themes/dark.yaml");
const HIGH_CONTRAST: &str = include_str!("../themes/high-contrast.yaml");

/// The name of the theme used when none is chosen, or the chosen one is missing.
pub const DEFAULT: &str = "Dark";

/// A color for each part of the window. See `themes/dark.yaml` for what each one colors.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub background: Color,
    pub sidebar: Color,
    pub menu_bar: Color,
    pub menu: Color,
    pub tab_active: Color,
    pub tab_inactive: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub field_background: Color,
    pub selection: Color,
    pub hover: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub status_bar: Color,
    pub status_text: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub border_width: f32,
    pub corner_radius: f32,
}

impl Colors {
    /// Sets the color named `role` to `value`.
    fn set(&mut self, role: &str, value: Color) -> Result<(), String> {
        let slot = match role {
            "background" => &mut self.background,
            "sidebar" => &mut self.sidebar,
            "menu_bar" => &mut self.menu_bar,
            "menu" => &mut self.menu,
            "tab_active" => &mut self.tab_active,
            "tab_inactive" => &mut self.tab_inactive,
            "border" => &mut self.border,
            "text" => &mut self.text,
            "text_muted" => &mut self.text_muted,
            "field_background" => &mut self.field_background,
            "selection" => &mut self.selection,
            "hover" => &mut self.hover,
            "accent" => &mut self.accent,
            "accent_text" => &mut self.accent_text,
            "status_bar" => &mut self.status_bar,
            "status_text" => &mut self.status_text,
            "danger" => &mut self.danger,
            "warning" => &mut self.warning,
            "success" => &mut self.success,
            _ => return Err(format!("unknown color {role:?}")),
        };
        *slot = value;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub colors: Colors,
}

impl Theme {
    /// The theme iced uses for widgets that are not styled by hand.
    pub fn to_iced(&self) -> iced::Theme {
        let c = &self.colors;
        iced::Theme::custom(
            self.name.clone(),
            iced::theme::Palette {
                background: c.background,
                text: c.text,
                primary: c.accent,
                success: c.success,
                warning: c.warning,
                danger: c.danger,
            },
        )
    }
}

/// The layout of a theme file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    #[serde(default)]
    base: Base,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    border_width: Option<f32>,
    corner_radius: Option<f32>,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Base {
    #[default]
    Dark,
    Light,
}

/// Reads a theme from YAML, filling in missing colors from `dark` or `light`.
fn parse(yaml: &str, dark: Option<&Theme>, light: Option<&Theme>) -> Result<Theme, String> {
    let file: ThemeFile = serde_yaml_ng::from_str(yaml).map_err(|error| error.to_string())?;

    let base = match file.base {
        Base::Dark => dark,
        Base::Light => light,
    };

    let mut colors = match base {
        Some(base) => base.colors,
        // Only the built-in themes have no base to fall back on, and they set every color.
        None => FALLBACK,
    };

    for (role, value) in &file.colors {
        let color =
            parse_color(value).ok_or_else(|| format!("{role}: {value:?} is not a color"))?;
        colors.set(role, color)?;
    }

    if let Some(width) = file.border_width {
        colors.border_width = width.clamp(0.0, 8.0);
    }
    if let Some(radius) = file.corner_radius {
        colors.corner_radius = radius.clamp(0.0, 24.0);
    }

    Ok(Theme {
        name: file.name.trim().to_string(),
        colors,
    })
}

/// Placeholder colors for a built-in theme to overwrite.
const FALLBACK: Colors = Colors {
    background: Color::BLACK,
    sidebar: Color::BLACK,
    menu_bar: Color::BLACK,
    menu: Color::BLACK,
    tab_active: Color::BLACK,
    tab_inactive: Color::BLACK,
    border: Color::WHITE,
    text: Color::WHITE,
    text_muted: Color::WHITE,
    field_background: Color::BLACK,
    selection: Color::BLACK,
    hover: Color::BLACK,
    accent: Color::WHITE,
    accent_text: Color::BLACK,
    status_bar: Color::BLACK,
    status_text: Color::WHITE,
    danger: Color::WHITE,
    warning: Color::WHITE,
    success: Color::WHITE,
    border_width: 1.0,
    corner_radius: 0.0,
};

/// Parses `#rgb`, `#rrggbb`, or `#rrggbbaa`.
pub fn parse_color(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();

    match hex.len() {
        3 => {
            let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|d| d * 17);
            Some(Color::from_rgb8(digit(0)?, digit(1)?, digit(2)?))
        }
        6 => Some(Color::from_rgb8(channel(0)?, channel(2)?, channel(4)?)),
        8 => Some(Color::from_rgba8(
            channel(0)?,
            channel(2)?,
            channel(4)?,
            f32::from(channel(6)?) / 255.0,
        )),
        _ => None,
    }
}

/// Formats `color` as `#rrggbb`, ignoring transparency.
pub fn to_hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The built-in themes.
pub fn built_in() -> Vec<Theme> {
    let dark = parse(DARK, None, None).expect("themes/dark.yaml is valid");
    let light = parse(HIGH_CONTRAST, None, None).expect("themes/high-contrast.yaml is valid");
    vec![dark, light]
}

/// The folder users put their own themes in, if the config folder can be found.
pub fn user_folder() -> Option<PathBuf> {
    crate::settings::config_folder().map(|folder| folder.join("themes"))
}

/// Loads the built-in themes, then every `.yaml` file in `folder`.
///
/// Returns the themes and a message for each file that could not be loaded.
/// A user theme with the same name as an earlier one replaces it.
pub fn load(folder: Option<&Path>) -> (Vec<Theme>, Vec<String>) {
    let mut themes = built_in();
    let mut problems = Vec::new();

    let Some(Ok(read_dir)) = folder.map(fs::read_dir) else {
        return (themes, problems);
    };

    let mut paths: Vec<PathBuf> = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml" || extension == "yml")
        })
        .collect();
    paths.sort();

    for path in paths {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        let theme = fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|yaml| parse(&yaml, Some(&themes[0]), Some(&themes[1])));

        match theme {
            Ok(theme) if theme.name.is_empty() => {
                problems.push(format!("{file_name}: the theme has no name"));
            }
            Ok(theme) => match themes.iter_mut().find(|t| t.name == theme.name) {
                Some(existing) => *existing = theme,
                None => themes.push(theme),
            },
            Err(error) => problems.push(format!("{file_name}: {error}")),
        }
    }

    (themes, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_load() {
        let themes = built_in();
        assert_eq!(themes[0].name, DEFAULT);
        assert_eq!(themes[1].name, "High Contrast");
    }

    #[test]
    fn parses_colors() {
        assert_eq!(parse_color("#fff"), Some(Color::WHITE));
        assert_eq!(parse_color("#000000"), Some(Color::BLACK));
        assert_eq!(parse_color(" #ff000080 ").unwrap().a, 128.0 / 255.0);
        assert_eq!(parse_color("ffffff"), None);
        assert_eq!(parse_color("#ggg"), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(to_hex(Color::from_rgb8(0x12, 0xab, 0xff)), "#12abff");
    }

    #[test]
    fn missing_colors_come_from_the_base() {
        let themes = built_in();
        let theme = parse(
            "name: Forest\nbase: light\ncolors:\n  accent: \"#228b22\"\n",
            Some(&themes[0]),
            Some(&themes[1]),
        )
        .unwrap();

        assert_eq!(theme.colors.accent, Color::from_rgb8(0x22, 0x8b, 0x22));
        assert_eq!(theme.colors.background, themes[1].colors.background);
    }

    #[test]
    fn rejects_bad_themes() {
        let themes = built_in();
        let parse = |yaml| parse(yaml, Some(&themes[0]), Some(&themes[1]));

        assert!(parse("name: X\ncolors:\n  backgrund: \"#000\"\n").is_err());
        assert!(parse("name: X\ncolors:\n  text: red\n").is_err());
        assert!(parse("colors: {}\n").is_err());
        assert!(parse("not yaml: [").is_err());
    }
}
