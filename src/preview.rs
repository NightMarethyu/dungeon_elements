//! The reading view: a note shown as formatted Markdown, the way GitHub shows a README.
//!
//! The note's name becomes its title, its other front matter fields a table of
//! properties, and its body is rendered with iced's Markdown widget. `[[Links]]` are turned
//! into Markdown links first so they can be clicked, and `![[picture.png]]` embeds show the
//! picture.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::advanced::text::Highlight;
use iced::widget::markdown::{self, HeadingLevel, Uri};
use iced::widget::text::Span;
use iced::widget::{Svg, column, container, image, rich_text, row, rule, scrollable, svg, text};
use iced::{Center, Element, Fill, Font, Length, border, font, padding};
use serde_yaml_ng::{Mapping, Value};

use crate::links::{self, Names};
use crate::note;
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Kind, title_case};

/// Links to notes are written `note:<target>`; embedded pictures `embed:<name>`.
const NOTE: &str = "note:";
const EMBED: &str = "embed:";

/// The file types shown as pictures.
const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// The widest the page gets, so lines stay easy to read on a wide window.
const PAGE_WIDTH: f32 = 860.0;

/// A note parsed for the reading view.
pub struct Preview {
    /// The text it was made from, to tell when it needs making again.
    source: String,
    /// The note's `name`, shown as a heading unless the body starts with one.
    title: Option<String>,
    /// The note's name and type, shown at the top of the box of fields.
    name: Option<String>,
    kind: Option<String>,
    /// The front matter fields worth showing, as (label, value).
    properties: Vec<(String, markdown::Content)>,
    body: markdown::Content,
    /// The files that pictures point to, by the URL in the Markdown.
    pictures: Arc<HashMap<Uri, PathBuf>>,
}

/// What clicking a link in the reading view does.
#[derive(Debug, Clone, PartialEq)]
pub enum Click {
    /// Opens the note a `[[link]]` names (or makes it).
    Note(String),
    /// Opens a web page in the browser.
    Web(String),
    /// Opens a file a Markdown link points to, like `[Map](maps/Town.md)`.
    File(PathBuf),
}

impl Preview {
    /// Parses the text of the note at `path` (which may not be saved yet), labelling its
    /// fields from `ty` when it has a type. Embedded pictures are looked for next to the
    /// note, then anywhere in the campaign folder `root`.
    pub fn new(source: &str, ty: Option<&ContentType>, path: &Path, root: &Path) -> Self {
        let (front, body) = match note::split_front_matter(source) {
            Ok((Some(yaml), body)) => match serde_yaml_ng::from_str::<Mapping>(yaml) {
                Ok(front) => (front, body),
                // Front matter that isn't keys and values shows as it is.
                Err(_) => (Mapping::new(), source),
            },
            Ok((None, body)) => (Mapping::new(), body),
            Err(_) => (Mapping::new(), source),
        };

        let starts_with_title = body
            .lines()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| line.starts_with("# "));
        let name = front
            .get("name")
            .map(note::value_to_text)
            .filter(|name| !name.trim().is_empty());
        let title = name.clone().filter(|_| !starts_with_title);

        let properties = properties(&front, ty)
            .into_iter()
            .map(|(label, value)| (label, markdown::Content::parse(&value)))
            .collect();

        let body = markdown::Content::parse(&with_markdown_links(body));
        let folder = path.parent().unwrap_or(root);
        let pictures = body
            .images()
            .iter()
            .filter_map(|url| Some((url.clone(), find_picture(url, folder, root)?)))
            .collect();

        Preview {
            source: source.to_string(),
            title,
            name,
            kind: ty.map(ContentType::label),
            properties,
            body,
            pictures: Arc::new(pictures),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// The page, producing the URL of each link that is clicked. `names` are the notes
    /// that exist, so links to missing ones can be dimmed like in the editor.
    pub fn view(&self, c: Colors, names: Names) -> Element<'_, Uri> {
        let reader = Reader {
            c,
            names,
            pictures: self.pictures.clone(),
        };
        let settings = markdown::Settings {
            code_size: 13.into(),
            ..markdown::Settings::with_text_size(15, markdown_style(c))
        };

        let mut page = column![].spacing(20);
        if let Some(title) = &self.title {
            page = page.push(
                column![
                    text(title.clone()).size(settings.h1_size).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::DEFAULT
                    }),
                    rule::horizontal(1).style(style::separator(c)),
                ]
                .spacing(8),
            );
        }

        let items = self.body.items();
        if self.properties.is_empty() {
            if !items.is_empty() {
                page = page.push(markdown::view_with(items, settings, &reader));
            } else if self.title.is_none() {
                page = page.push(
                    text("This note is empty. Switch to Edit to write in it.")
                        .size(settings.text_size)
                        .color(c.text_muted),
                );
            }
        } else {
            // Like a wiki's infobox, the fields sit in a box on the right, beside the
            // note's opening text; the rest of the note runs full width under them.
            let lead = lead_length(items);
            page = page.push(
                row![
                    container(markdown::view_with(&items[..lead], settings, &reader))
                        .width(Length::FillPortion(1)),
                    container(self.infobox(c, settings, &reader)).width(Length::FillPortion(1)),
                ]
                .spacing(24),
            );
            if lead < items.len() {
                page = page.push(markdown::view_with(&items[lead..], settings, &reader));
            }
        }

        scrollable(
            container(container(page).max_width(PAGE_WIDTH))
                .center_x(Fill)
                .padding([28, 36]),
        )
        .height(Fill)
        .into()
    }
}

impl Preview {
    /// The box of the note's fields: its name and type across the top, then a row for
    /// each field.
    fn infobox<'a>(
        &'a self,
        c: Colors,
        settings: markdown::Settings,
        reader: &Reader,
    ) -> Element<'a, Uri> {
        let mut header = column![].spacing(2).align_x(Center).width(Fill);
        if let Some(name) = &self.name {
            header = header.push(
                text(name.clone())
                    .size(settings.text_size * 1.2)
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::DEFAULT
                    })
                    .align_x(Center),
            );
        }
        if let Some(kind) = &self.kind {
            header = header.push(
                text(kind.clone())
                    .size(settings.text_size * 0.85)
                    .color(c.text_muted),
            );
        }

        let rows = self.properties.iter().map(|(label, value)| {
            row![
                text(label.clone())
                    .size(settings.text_size)
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::DEFAULT
                    })
                    .width(Length::FillPortion(2)),
                container(markdown::view_with(value.items(), settings, reader))
                    .width(Length::FillPortion(3)),
            ]
            .spacing(12)
            .into()
        });

        let mut infobox = column![].spacing(10);
        if self.name.is_some() || self.kind.is_some() {
            infobox = infobox.push(
                container(header)
                    .padding([8, 12])
                    .width(Fill)
                    .style(style::infobox_header(c)),
            );
        }
        infobox = infobox.push(container(column(rows).spacing(8)).padding([0, 14]));

        container(infobox)
            .padding(padding::bottom(12))
            .width(Fill)
            .style(style::card(c))
            .into()
    }
}

/// How many of a note's first Markdown items go beside the box of fields: everything
/// before its second heading (or its first, if the note doesn't start with one).
fn lead_length(items: &[markdown::Item]) -> usize {
    let starts_with_heading = matches!(items.first(), Some(markdown::Item::Heading(..)));
    items
        .iter()
        .enumerate()
        .skip(usize::from(starts_with_heading))
        .find(|(_, item)| matches!(item, markdown::Item::Heading(..)))
        .map_or(items.len(), |(index, _)| index)
}

/// What clicking the link to `url`, in the note at `path`, should do.
pub fn click(url: &str, path: &Path) -> Option<Click> {
    if let Some(target) = url.strip_prefix(NOTE) {
        return Some(Click::Note(target.to_string()));
    }
    let lower = url.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Some(Click::Web(url.to_string()));
    }
    // Other schemes, and links to headings on the same page.
    if url.starts_with('#') || url.contains("://") {
        return None;
    }

    let relative = decode(url.split('#').next().unwrap_or(url));
    let file = path.parent()?.join(relative);
    if file.is_file() {
        return Some(Click::File(file));
    }
    // Obsidian writes Markdown links to notes without `.md` too.
    let note = file.with_extension("md");
    note.is_file().then_some(Click::File(note))
}

/// The front matter fields to show as properties, as (label, Markdown value).
///
/// The type's fields come first, in its order and with its labels; then the note's own.
/// The name (shown as the title), long text fields (which are body sections), the keys
/// the app manages, and empty values are left out.
fn properties(front: &Mapping, ty: Option<&ContentType>) -> Vec<(String, String)> {
    let fields = ty.map(|ty| ty.fields.as_slice()).unwrap_or_default();
    let mut shown = Vec::new();

    for field in fields {
        if field.key == "name" || field.kind == Kind::Longtext {
            continue;
        }
        if let Some(value) = front.get(field.key.as_str()).and_then(value_markdown) {
            shown.push((field.label(), value));
        }
    }

    for (key, value) in front {
        let Value::String(key) = key else {
            continue;
        };
        if key == "name"
            || crate::form::is_reserved(key)
            || fields.iter().any(|field| &field.key == key)
        {
            continue;
        }
        if let Some(value) = value_markdown(value) {
            let label = if key.chars().any(char::is_uppercase) || key.contains(' ') {
                key.clone()
            } else {
                title_case(key)
            };
            shown.push((label, value));
        }
    }

    shown
}

/// A front matter value as one line of Markdown, or `None` if it is empty.
fn value_markdown(value: &Value) -> Option<String> {
    let text = match value {
        Value::Bool(true) => "Yes".to_string(),
        Value::Bool(false) => "No".to_string(),
        value => note::value_to_text(value),
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // A value is plain text, so it mustn't start a heading, list or quote.
    let block_start = text.starts_with(['#', '>', '-', '+', '*', '=', '|', '`', '~'])
        || text
            .find(|c: char| !c.is_ascii_digit())
            .is_some_and(|end| end > 0 && text[end..].starts_with(['.', ')']));
    let text = with_markdown_links(&text.replace('\n', " "));
    Some(if block_start {
        format!("\\{text}")
    } else {
        text
    })
}

/// Rewrites the `[[links]]` in `text` as Markdown links the reading view can click:
/// `[[Note|shown]]` becomes `[shown](<note:Note>)`, and `![[picture.png]]` a picture.
/// Embeds of notes show as links to them.
pub fn with_markdown_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for link in links::find(text) {
        let whole = &text[link.range.clone()];
        let target = &text[link.target.clone()];
        let embed = whole.starts_with('!');
        let inner = whole
            .trim_start_matches('!')
            .trim_start_matches("[[")
            .trim_end_matches("]]");
        let shown = match inner.split_once('|') {
            Some((_, shown)) => shown.trim().to_string(),
            // Obsidian shows `[[Note#Heading]]` as "Note > Heading".
            None => inner.trim().replace('#', " > "),
        };
        if target.is_empty() || target.contains(['<', '>']) {
            continue;
        }

        out.push_str(&text[last..link.range.start]);
        if embed && is_picture(Path::new(target)) {
            out.push_str(&format!("![{}](<{EMBED}{target}>)", escape(&shown)));
        } else {
            out.push_str(&format!("[{}](<{NOTE}{target}>)", escape(&shown)));
        }
        last = link.range.end;
    }

    out.push_str(&text[last..]);
    out
}

/// Escapes the characters that would change how link text is formatted.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '[' | ']' | '*' | '_' | '`' | '\\' | '<') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| PICTURES.contains(&extension.to_ascii_lowercase().as_str()))
}

/// The file a picture's URL points to: a path next to the note, or for an embed, a file
/// of that name next to the note, at that path in the campaign, or anywhere in it.
fn find_picture(url: &str, folder: &Path, root: &Path) -> Option<PathBuf> {
    if let Some(name) = url.strip_prefix(EMBED) {
        return [folder.join(name), root.join(name)]
            .into_iter()
            .find(|path| path.is_file())
            .or_else(|| {
                let file_name = Path::new(name).file_name()?.to_str()?;
                find_file(root, file_name, 0)
            });
    }
    if url.contains("://") {
        return None;
    }
    let path = folder.join(decode(url));
    (path.is_file() && is_picture(&path)).then_some(path)
}

/// Looks through `folder` and its subfolders for a file called `name`, ignoring case.
fn find_file(folder: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    if depth > 8 {
        return None;
    }
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            folders.push(path);
        } else if file_name.eq_ignore_ascii_case(name) {
            return Some(path);
        }
    }
    folders
        .iter()
        .find_map(|folder| find_file(folder, name, depth + 1))
}

/// Decodes the `%20`-style escapes in a link's path.
fn decode(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = url.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn markdown_style(c: Colors) -> markdown::Style {
    markdown::Style {
        font: Font::DEFAULT,
        inline_code_highlight: Highlight {
            background: c.field_background.into(),
            border: border::rounded(4),
        },
        inline_code_padding: padding::left(3).right(3),
        inline_code_color: c.text,
        inline_code_font: Font::MONOSPACE,
        code_block_font: Font::MONOSPACE,
        link_color: c.accent,
    }
}

/// Shows Markdown in the theme's colors.
struct Reader {
    c: Colors,
    names: Names,
    pictures: Arc<HashMap<Uri, PathBuf>>,
}

impl<'a> markdown::Viewer<'a, Uri> for Reader {
    fn on_link_click(url: Uri) -> Uri {
        url
    }

    fn image(
        &self,
        settings: markdown::Settings,
        url: &'a Uri,
        _title: &'a str,
        alt: &markdown::Text,
    ) -> Element<'a, Uri> {
        match self.pictures.get(url) {
            Some(path)
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("svg")) =>
            {
                Svg::new(svg::Handle::from_path(path))
                    .width(Length::Shrink)
                    .into()
            }
            Some(path) => image(image::Handle::from_path(path)).into(),
            None => {
                let what = if url.contains("://") {
                    "Pictures from the web aren't shown"
                } else {
                    "Picture not found"
                };
                container(
                    column![
                        rich_text(alt.spans(settings.style)).size(settings.text_size),
                        text(format!("{what}: {}", url.trim_start_matches(EMBED)))
                            .size(settings.text_size * 0.85)
                            .color(self.c.text_muted),
                    ]
                    .spacing(4),
                )
                .padding([8, 12])
                .style(style::card(self.c))
                .into()
            }
        }
    }

    fn heading(
        &self,
        settings: markdown::Settings,
        level: &'a HeadingLevel,
        text: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Uri> {
        let heading = markdown::heading(settings, level, text, index, Self::on_link_click);
        // Like GitHub, the two biggest headings are underlined.
        match level {
            HeadingLevel::H1 | HeadingLevel::H2 => {
                column![heading, rule::horizontal(1).style(style::separator(self.c))]
                    .spacing(6)
                    .into()
            }
            _ => heading,
        }
    }

    fn paragraph(&self, settings: markdown::Settings, text: &markdown::Text) -> Element<'a, Uri> {
        // Links to notes that don't exist yet are dimmed, as in the editor. In themes
        // where links are the text's color, they are underlined to stand out.
        let underline = self.c.accent == self.c.text;
        let spans: Vec<Span<'static, Uri>> = text
            .spans(settings.style)
            .iter()
            .map(|span| match &span.link {
                Some(url) => {
                    let missing = url
                        .strip_prefix(NOTE)
                        .is_some_and(|target| !self.names.contains(target));
                    let span = span.clone().underline(underline);
                    if missing {
                        span.color(self.c.text_muted)
                    } else {
                        span
                    }
                }
                None => span.clone(),
            })
            .collect();

        rich_text(spans)
            .size(settings.text_size)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, Uri> {
        let lines = column(lines.iter().map(|line| {
            rich_text(line.spans(settings.style))
                .on_link_click(Self::on_link_click)
                .font(settings.style.code_block_font)
                .size(settings.code_size)
                .into()
        }));

        container(
            scrollable(container(lines).padding(settings.code_size)).direction(
                scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default()
                        .width(settings.code_size / 2)
                        .scroller_width(settings.code_size / 2),
                ),
            ),
        )
        .width(Fill)
        .style(style::card(self.c))
        .into()
    }

    fn quote(
        &self,
        settings: markdown::Settings,
        contents: &'a [markdown::Item],
    ) -> Element<'a, Uri> {
        row![
            rule::vertical(4).style(style::separator(self.c)),
            container(
                column(
                    contents
                        .iter()
                        .enumerate()
                        .map(|(i, item)| markdown::item(self, settings, item, i)),
                )
                .spacing(settings.spacing.0),
            )
            .style(style::muted_text(self.c)),
        ]
        .height(Length::Shrink)
        .spacing(settings.spacing.0)
        .into()
    }

    fn unordered_list(
        &self,
        settings: markdown::Settings,
        bullets: &'a [markdown::Bullet],
    ) -> Element<'a, Uri> {
        // Like iced's own list, but task boxes are drawn here: iced's checkbox draws its
        // tick outside the scrolled page.
        let size = settings.text_size;
        column(bullets.iter().map(|bullet| {
            let (marker, items): (Element<'a, Uri>, _) = match bullet {
                markdown::Bullet::Point { items } => (text("•").size(size).into(), items),
                markdown::Bullet::Task { items, done } => {
                    let tick = text(if *done { "✓" } else { "" })
                        .size(size * 0.8)
                        .color(self.c.accent_text)
                        .center();
                    let check = container(tick)
                        .center(size * 1.1)
                        .style(style::task_box(self.c, *done));
                    (
                        container(check)
                            .center_y(text::LineHeight::default().to_absolute(size))
                            .into(),
                        items,
                    )
                }
            };
            row![
                marker,
                markdown::view_with(
                    items,
                    markdown::Settings {
                        spacing: settings.spacing * 0.6,
                        ..settings
                    },
                    self,
                )
            ]
            .spacing(settings.spacing)
            .into()
        }))
        .spacing(settings.spacing * 0.75)
        .padding([0.0, settings.spacing.0])
        .into()
    }

    fn rule(&self, _settings: markdown::Settings) -> Element<'a, Uri> {
        rule::horizontal(2).style(style::separator(self.c)).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_wiki_links() {
        assert_eq!(
            with_markdown_links("Ask [[Old Tom]] or [[npcs/Ann|the smith]]."),
            "Ask [Old Tom](<note:Old Tom>) or [the smith](<note:npcs/Ann>)."
        );
        assert_eq!(
            with_markdown_links("See [[Keep#Gate]] and ![[Keep]]"),
            "See [Keep > Gate](<note:Keep>) and [Keep](<note:Keep>)"
        );
        assert_eq!(
            with_markdown_links("![[maps/Town.png]]"),
            "![maps/Town.png](<embed:maps/Town.png>)"
        );
        // Code is left alone.
        assert_eq!(with_markdown_links("`[[x]]`"), "`[[x]]`");
    }

    #[test]
    fn shows_fields_as_properties() {
        let ty: ContentType = crate::types::parse(
            "type: npc\nfields:\n  - key: name\n  - key: home\n    label: Lives in\n    kind: link\n  - key: history\n    kind: longtext\n",
        )
        .unwrap();
        let front: Mapping = serde_yaml_ng::from_str(
            "type: npc\nname: Ann\nhome: '[[Town]]'\nmood: '# grumpy'\nalive: true\nempty: ''\n",
        )
        .unwrap();

        assert_eq!(
            properties(&front, Some(&ty)),
            vec![
                ("Lives in".to_string(), "[Town](<note:Town>)".to_string()),
                ("Mood".to_string(), "\\# grumpy".to_string()),
                ("Alive".to_string(), "Yes".to_string()),
            ]
        );
    }

    #[test]
    fn titles_with_the_name_unless_the_body_has_one() {
        let path = Path::new("/campaign/npcs/Ann.md");
        let root = Path::new("/campaign");
        let named = Preview::new("---\nname: Ann\n---\nHello\n", None, path, root);
        assert_eq!(named.title.as_deref(), Some("Ann"));

        let titled = Preview::new("---\nname: Ann\n---\n# Ann the Smith\n", None, path, root);
        assert_eq!(titled.title, None);
    }

    #[test]
    fn puts_the_opening_text_beside_the_fields() {
        let items: Vec<_> = markdown::parse("Intro.\n\nMore.\n\n## History\n\nOld.\n").collect();
        assert_eq!(lead_length(&items), 2);

        let items: Vec<_> = markdown::parse("## Appearance\n\nTall.\n\n## Notes\n\nx\n").collect();
        assert_eq!(lead_length(&items), 2);

        let items: Vec<_> = markdown::parse("Just text.\n").collect();
        assert_eq!(lead_length(&items), 1);
    }

    #[test]
    fn follows_links() {
        let path = Path::new("/campaign/npcs/Ann.md");
        assert_eq!(click("note:Town", path), Some(Click::Note("Town".into())));
        assert_eq!(
            click("https://example.com", path),
            Some(Click::Web("https://example.com".into()))
        );
        assert_eq!(click("#history", path), None);
        assert_eq!(decode("Old%20Tom.md"), "Old Tom.md");
    }
}
