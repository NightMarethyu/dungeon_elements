//! The reading view: a note shown as formatted Markdown, the way GitHub shows a README.
//!
//! The note's name becomes its title, its other front matter fields a box of properties
//! like a wiki's infobox, and its body is rendered with iced's Markdown widget. `[[Links]]`
//! are turned into Markdown links first so they can be clicked, `![[picture.png]]` embeds
//! show the picture, and `![[Note]]` embeds show the other note's text in a box. Pictures
//! can also come from the web, and code blocks are colored by their language.
//!
//! The page can be edited in place: the body is split into blocks (a paragraph, a list, a
//! table...), and clicking one or a field asks for an editor to show in its place.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::advanced::text::Highlight;
use iced::mouse::Interaction;
use iced::widget::markdown::{self, HeadingLevel, Uri};
use iced::widget::text::Span;
use iced::widget::{
    Svg, column, container, image, mouse_area, rich_text, row, rule, scrollable, svg, text,
};
use iced::{Center, Element, Fill, Font, Length, border, font, padding};
use serde_yaml_ng::{Mapping, Value};

use crate::code;
use crate::dice::{self, Dice, Expression};
use crate::links::{self, Index, Names};
use crate::note;
use crate::style;
use crate::tables;
use crate::theme::Colors;
use crate::types::{ContentType, Field, Kind, title_case};
use crate::web;

/// Links to notes are written `note:<target>`; embedded pictures `embed:<name>`, and
/// embedded notes `embed-note:<target>`, with any `#heading`.
const NOTE: &str = "note:";
const EMBED: &str = "embed:";
const EMBED_NOTE: &str = "embed-note:";
/// Clicking a task box sends `task:<where its [ ] is in the note's text>`.
const TASK: &str = "task:";

/// The file types shown as pictures.
pub const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

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
    properties: Vec<Property>,
    blocks: Vec<Block>,
    /// The files that pictures point to, by the URL in the Markdown.
    pictures: HashMap<Uri, PathBuf>,
    /// The notes embedded in this one, by the URL in the Markdown.
    embeds: HashMap<Uri, Embed>,
    /// The colored parts of each code block, by a hash of its language and code, made
    /// the first time the block is shown. `None` for a language without colors.
    code: RefCell<HashMap<u64, Option<Vec<code::Line>>>>,
}

/// A note shown inside another, with `![[Note]]` or `![[Note#Heading]]`.
struct Embed {
    /// The note's name and heading, shown above it.
    title: String,
    /// The link to the note, to open it.
    target: String,
    /// The note's text (or the heading's section), or why it can't be shown.
    content: Result<markdown::Content, String>,
}

/// A front matter field in the box of fields.
struct Property {
    key: String,
    label: String,
    /// `None` for a field of the note's type that is empty.
    value: Option<markdown::Content>,
    /// The dice in its value, like `1d8+3`, `+5` or a dice field's `1d20+5 advantage`, each
    /// with a button to roll it.
    rolls: Vec<String>,
    /// The roll tables the field rolls on to fill it in, and whether its button reads
    /// Inspire me (a long text field with one table).
    tables: Vec<String>,
    inspire: bool,
}

/// A paragraph, heading, list, table, or other top-level piece of the note's body.
struct Block {
    /// Where it is in the note's text.
    range: Range<usize>,
    content: markdown::Content,
    /// Where the `[ ]` of each of its task boxes is in the note's text, in order.
    tasks: Vec<usize>,
    /// The roll table it is, if it is one.
    table: Option<RollTable>,
}

/// What the bar above a roll table needs: its die, and whether it has columns to roll
/// on their own.
struct RollTable {
    die: String,
    has_columns: bool,
}

/// A note that links to the one shown, for the list under it ("Linked from").
#[derive(Debug, Clone, PartialEq)]
pub struct Backlink {
    /// What kind of note it is (its folder's type), to list it with others like it.
    pub group: String,
    pub name: String,
    /// The link to it from the note shown, to open it.
    pub link: String,
    /// Where it links to the note shown: the lines, or the fields with their values.
    pub lines: Vec<String>,
}

/// The most places shown for each note in the list of backlinks.
const BACKLINK_LINES: usize = 3;

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

/// The messages the page sends, and the editor shown in place of what is being edited.
pub struct Editing<'a, M> {
    /// A link was clicked.
    pub on_link: fn(Uri) -> M,
    /// Block `index` was clicked.
    pub on_block: fn(usize) -> M,
    /// The task box whose `[ ]` is at this place in the note's text was clicked.
    pub on_task: fn(usize) -> M,
    /// The field with this front matter key was clicked (the title is `name`).
    pub on_field: fn(String) -> M,
    /// The roll button of a field was clicked: the field's label, and the dice to roll.
    pub on_roll: fn(String, String) -> M,
    /// The icon on the roll buttons.
    pub roll_icon: svg::Handle,
    /// The Roll button of the roll table at this place in the note's text was clicked.
    pub on_roll_table: fn(usize) -> M,
    /// The roll button of the field with this key, which rolls on this roll table.
    pub on_roll_into: fn(String, String) -> M,
    /// The roll table at this place was set to roll each column on its own, or not.
    pub on_each_column: fn(usize, bool) -> M,
    /// Where the roll tables that roll each column on their own are in the note's text.
    pub each_column: Vec<usize>,
    /// The space for adding text at the end was clicked.
    pub on_add: M,
    /// Somewhere else on the page was clicked.
    pub on_page: M,
    /// The editor for block `index` (or, at the number of blocks, for new text at the end).
    pub block: Option<(usize, Element<'a, M>)>,
    /// The input for the field with this key.
    pub field: Option<(String, Element<'a, M>)>,
}

impl Preview {
    /// Parses the text of the note at `path` (which may not be saved yet), labelling its
    /// fields from `ty` when it has a type. Embedded pictures are looked for next to the
    /// note, then anywhere in the campaign folder `root`; embedded notes in `index`.
    pub fn new(
        source: &str,
        ty: Option<&ContentType>,
        path: &Path,
        root: &Path,
        index: &Index,
    ) -> Self {
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
            .map(|(key, label, value)| Property {
                rolls: rolls(&front, &key, ty),
                tables: field(ty, &key)
                    .map(|field| field.tables().into_iter().map(String::from).collect())
                    .unwrap_or_default(),
                inspire: field(ty, &key)
                    .is_some_and(|field| field.tables().len() == 1 && field.kind == Kind::Longtext),
                key,
                label,
                value: value.map(|value| markdown::Content::parse(&value)),
            })
            .collect();

        // `body` is the end of `source`, so this is where it starts in it.
        let offset = source.len() - body.len();
        let roll_tables = tables::find(source);
        let blocks: Vec<Block> = block_ranges(body)
            .into_iter()
            .map(|range| Block {
                content: markdown::Content::parse(&pictures_apart(&rewrite_links(
                    &body[range.clone()],
                    true,
                ))),
                tasks: task_boxes(&body[range.clone()])
                    .into_iter()
                    .map(|at| at + range.start + offset)
                    .collect(),
                table: roll_tables
                    .iter()
                    .find(|table| table.range.start == range.start + offset)
                    .map(|table| RollTable {
                        die: table.die.to_string(),
                        has_columns: table.has_columns(),
                    }),
                range: range.start + offset..range.end + offset,
            })
            .collect();

        let folder = path.parent().unwrap_or(root);
        let mut pictures: HashMap<Uri, PathBuf> = HashMap::new();
        let mut embeds = HashMap::new();
        for url in blocks.iter().flat_map(|block| block.content.images()) {
            if let Some(link) = url.strip_prefix(EMBED_NOTE) {
                let embed = Embed::new(link, body, path, index);
                let target = link.split('#').next().unwrap_or(link);
                if let (Ok(content), Some(file)) = (&embed.content, index.resolve(target, path)) {
                    // Pictures in the embedded note are found from its own folder.
                    let folder = file.parent().unwrap_or(root);
                    for url in content.images() {
                        if let Some(file) = find_picture(url, folder, root) {
                            pictures.entry(url.clone()).or_insert(file);
                        }
                    }
                }
                embeds.insert(url.clone(), embed);
            } else if let Some(file) = find_picture(url, folder, root) {
                pictures.insert(url.clone(), file);
            }
        }

        Preview {
            source: source.to_string(),
            title,
            name,
            kind: ty.map(ContentType::label),
            properties,
            blocks,
            pictures,
            embeds,
            code: RefCell::default(),
        }
    }

    /// Whether other notes are shown in this one.
    pub fn has_embeds(&self) -> bool {
        !self.embeds.is_empty()
    }

    /// The web addresses of the pictures in the note, and in the notes it embeds.
    pub fn web_pictures(&self) -> impl Iterator<Item = &str> {
        let embedded = self
            .embeds
            .values()
            .filter_map(|embed| embed.content.as_ref().ok());
        self.blocks
            .iter()
            .map(|block| &block.content)
            .chain(embedded)
            .flat_map(|content| content.images())
            .map(String::as_str)
            .filter(|url| web::is_web(url))
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Where block `index` is in the note's text, or `None` if there is no such block.
    pub fn block_range(&self, index: usize) -> Option<Range<usize>> {
        self.blocks.get(index).map(|block| block.range.clone())
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// The page. `names` are the notes that exist, so links to missing ones can be dimmed
    /// like in the editor; `backlinks` are the notes that link to this one, listed at the end.
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        names: Names,
        web: &'a web::Pictures,
        backlinks: Vec<Backlink>,
        mut edit: Editing<'a, M>,
    ) -> Element<'a, M> {
        let reader = Reader {
            c,
            names,
            pictures: &self.pictures,
            embeds: &self.embeds,
            web,
            code: &self.code,
            tasks: &[],
            next_task: Cell::new(0),
        };
        let settings = markdown::Settings {
            code_size: 13.into(),
            ..markdown::Settings::with_text_size(15, markdown_style(c))
        };

        let mut page = column![].spacing(20);
        let title = match edit.field.take() {
            Some((key, input)) if key == "name" => Some(input),
            field => {
                edit.field = field;
                self.title.as_ref().map(|title| {
                    let title = text(title.clone()).size(settings.h1_size).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::DEFAULT
                    });
                    mouse_area(title)
                        .on_press((edit.on_field)("name".into()))
                        .interaction(Interaction::Text)
                        .into()
                })
            }
        };
        if let Some(title) = title {
            page = page
                .push(column![title, rule::horizontal(1).style(style::separator(c))].spacing(8));
        }

        let blocks = |range: Range<usize>, edit: &mut Editing<'a, M>| {
            column(range.map(|index| self.block(index, settings, &reader, edit)))
                .spacing(settings.spacing)
        };

        if self.properties.is_empty() {
            page = page.push(blocks(0..self.blocks.len(), &mut edit));
        } else {
            // Like a wiki's infobox, the fields sit in a box on the right, beside the
            // note's opening text; the rest of the note runs full width under them.
            let lead = self.lead_length();
            let beside = blocks(0..lead, &mut edit);
            let infobox = self.infobox(c, settings, &reader, &mut edit);
            page = page.push(
                row![
                    container(beside).width(Length::FillPortion(1)),
                    container(infobox).width(Length::FillPortion(1)),
                ]
                .spacing(24),
            );
            page = page.push(blocks(lead..self.blocks.len(), &mut edit));
        }

        // Text added at the end goes here.
        let add: Element<'a, M> = match edit.block.take() {
            Some((index, editor)) if index >= self.blocks.len() => editor,
            _ => {
                let hint = if self.blocks.is_empty() {
                    "This note has no text yet. Click here to write."
                } else {
                    "Click here to add more."
                };
                mouse_area(
                    container(text(hint).size(settings.text_size).color(c.text_muted))
                        .padding(padding::top(8).bottom(40))
                        .width(Fill),
                )
                .on_press(edit.on_add.clone())
                .interaction(Interaction::Text)
                .into()
            }
        };
        page = page.push(add);
        if !backlinks.is_empty() {
            page = page.push(linked_from(c, settings, backlinks).map(edit.on_link));
        }

        scrollable(
            mouse_area(
                container(container(page).max_width(PAGE_WIDTH))
                    .center_x(Fill)
                    .padding([28, 36]),
            )
            .on_press(edit.on_page),
        )
        .height(Fill)
        .into()
    }

    /// Block `index`, or its editor if it is being edited.
    fn block<'a, M: Clone + 'a>(
        &'a self,
        index: usize,
        settings: markdown::Settings,
        reader: &Reader<'a>,
        edit: &mut Editing<'a, M>,
    ) -> Element<'a, M> {
        match edit.block.take() {
            Some((editing, editor)) if editing == index => return editor,
            other => edit.block = other,
        }

        let block = &self.blocks[index];
        // This block's task boxes can be ticked; they are drawn in the order they are in.
        let reader = Reader {
            tasks: &block.tasks,
            next_task: Cell::new(0),
            ..reader.clone()
        };
        let (on_link, on_task) = (edit.on_link, edit.on_task);
        let items = block.content.items();
        let mut block = container(markdown::view_with(items, settings, &reader).map(
            move |url: Uri| match url.strip_prefix(TASK).and_then(|at| at.parse().ok()) {
                Some(at) => on_task(at),
                None => on_link(url),
            },
        ))
        .width(Fill);
        // Each block is laid out on its own, so headings get their space above here.
        if index > 0 && matches!(items.first(), Some(markdown::Item::Heading(..))) {
            block = block.padding(padding::top(settings.text_size / 2.0));
        }

        let block_element: Element<'a, M> = mouse_area(block)
            .on_press((edit.on_block)(index))
            .interaction(Interaction::Text)
            .into();
        let Some(table) = &self.blocks[index].table else {
            return block_element;
        };
        // A roll table gets a bar above it to roll it.
        let start = self.blocks[index].range.start;
        let roll = dice::field_button(reader.c, &edit.roll_icon, &table.die, {
            let on_roll = edit.on_roll_table;
            move |_| on_roll(start)
        });
        let mut bar = row![roll].spacing(12).align_y(Center);
        if table.has_columns {
            let on_each = edit.on_each_column;
            bar = bar.push(tables::each_column(
                reader.c,
                edit.each_column.contains(&start),
                move |on| on_each(start, on),
            ));
        }
        column![bar, block_element].spacing(6).into()
    }

    /// How many of the note's first blocks go beside the box of fields: everything
    /// before its second heading (or its first, if the note doesn't start with one).
    fn lead_length(&self) -> usize {
        let is_heading = |block: &Block| {
            matches!(
                block.content.items().first(),
                Some(markdown::Item::Heading(..))
            )
        };
        let starts_with_heading = self.blocks.first().is_some_and(is_heading);
        self.blocks
            .iter()
            .enumerate()
            .skip(usize::from(starts_with_heading))
            .find(|(_, block)| is_heading(block))
            .map_or(self.blocks.len(), |(index, _)| index)
    }

    /// The box of the note's fields: its name and type across the top, then a row for
    /// each field. Clicking a field edits it.
    fn infobox<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        settings: markdown::Settings,
        reader: &Reader<'a>,
        edit: &mut Editing<'a, M>,
    ) -> Element<'a, M> {
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

        let rows = self.properties.iter().map(|property| {
            let label = text(property.label.clone())
                .size(settings.text_size)
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Font::DEFAULT
                })
                .width(Length::FillPortion(2));

            match edit.field.take() {
                Some((key, input)) if key == property.key => {
                    return row![label, container(input).width(Length::FillPortion(3))]
                        .spacing(12)
                        .align_y(Center)
                        .into();
                }
                other => edit.field = other,
            }

            let value: Element<'a, M> = match &property.value {
                Some(value) => {
                    markdown::view_with(value.items(), settings, reader).map(edit.on_link)
                }
                None => text("—")
                    .size(settings.text_size)
                    .color(c.text_muted)
                    .into(),
            };
            // The roll buttons go under the value; clicking one rolls instead of editing.
            let value: Element<'a, M> = if property.rolls.is_empty() && property.tables.is_empty() {
                value
            } else {
                let on_roll = edit.on_roll;
                let mut buttons: Vec<Element<'a, M>> = property
                    .rolls
                    .iter()
                    .map(|expression| {
                        let label = property.label.clone();
                        dice::field_button(c, &edit.roll_icon, expression, move |expression| {
                            on_roll(label.clone(), expression)
                        })
                    })
                    .collect();
                for table in &property.tables {
                    buttons.push(dice::table_button(
                        c,
                        &edit.roll_icon,
                        table,
                        property.inspire,
                        (edit.on_roll_into)(property.key.clone(), table.clone()),
                    ));
                }
                column![value, row(buttons).spacing(6).wrap()]
                    .spacing(6)
                    .into()
            };
            mouse_area(row![label, container(value).width(Length::FillPortion(3))].spacing(12))
                .on_press((edit.on_field)(property.key.clone()))
                .interaction(Interaction::Pointer)
                .into()
        });
        let rows: Vec<Element<'a, M>> = rows.collect();

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

/// Splits a note's body into its top-level blocks, as byte ranges without the blank
/// lines between them.
/// The field of `ty` with this key.
fn field<'a>(ty: Option<&'a ContentType>, key: &str) -> Option<&'a Field> {
    ty?.fields.iter().find(|field| field.key == key)
}

fn block_ranges(body: &str) -> Vec<Range<usize>> {
    use pulldown_cmark::{Event, Options, Parser};

    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut ranges = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(_) => {
                if depth == 0 {
                    start = range.start;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    ranges.push(start..range.end);
                }
            }
            // Rules and the like stand alone.
            _ if depth == 0 => ranges.push(range),
            _ => {}
        }
    }

    ranges
        .into_iter()
        .map(|range| {
            let trimmed = body[range.clone()].trim_end();
            range.start..range.start + trimmed.len()
        })
        .filter(|range| !range.is_empty())
        .collect()
}

/// Where the `[ ]` or `[x]` of each task box in `text` is, in order.
fn task_boxes(text: &str) -> Vec<usize> {
    use pulldown_cmark::{Event, Options, Parser};

    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    Parser::new_ext(text, options)
        .into_offset_iter()
        .filter(|(event, _)| matches!(event, Event::TaskListMarker(_)))
        .map(|(_, range)| range.start)
        .collect()
}

/// The note's `text` with the task box whose `[ ]` is at `at` ticked, or unticked if it
/// was. `None` if there is no task box there.
pub fn toggle_task(text: &str, at: usize) -> Option<String> {
    let tick = match text.get(at..at + 3)? {
        "[ ]" => "x",
        "[x]" | "[X]" => " ",
        _ => return None,
    };
    let mut text = text.to_string();
    text.replace_range(at + 1..at + 2, tick);
    Some(text)
}

/// The list of the notes that link to the one shown, under a "Linked from" heading, in
/// groups by kind. Each note's name opens it; under it are the places it links here.
fn linked_from<'a>(
    c: Colors,
    settings: markdown::Settings,
    backlinks: Vec<Backlink>,
) -> Element<'a, Uri> {
    let bold = Font {
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    };
    let count = backlinks.len();
    let heading = row![
        text("Linked from").size(settings.h3_size).font(bold),
        text(if count == 1 {
            "1 note".to_string()
        } else {
            format!("{count} notes")
        })
        .size(settings.text_size * 0.85)
        .color(c.text_muted),
    ]
    .spacing(10)
    .align_y(iced::alignment::Vertical::Bottom);

    let mut groups = column![].spacing(18);
    let mut backlinks = backlinks.into_iter().peekable();
    while let Some(first) = backlinks.next() {
        let group = first.group.clone();
        let mut notes = column![].spacing(10);
        let mut next = Some(first);
        while let Some(backlink) = next {
            let name = rich_text([Span::new(backlink.name)
                .link(format!("{NOTE}{}", backlink.link))
                .color(c.accent)
                .font(bold)])
            .size(settings.text_size)
            .on_link_click(|url: Uri| url);
            let more = backlink.lines.len().saturating_sub(BACKLINK_LINES);
            let mut lines: Vec<Element<'a, Uri>> = backlink
                .lines
                .into_iter()
                .take(BACKLINK_LINES)
                .map(|line| {
                    text(line)
                        .size(settings.text_size * 0.9)
                        .color(c.text_muted)
                        .into()
                })
                .collect();
            if more > 0 {
                lines.push(
                    text(format!("and {more} more"))
                        .size(settings.text_size * 0.8)
                        .color(c.text_muted)
                        .into(),
                );
            }
            notes = notes.push(column![name, column(lines).spacing(2)].spacing(2));
            next = backlinks.next_if(|backlink| backlink.group == group);
        }
        groups = groups.push(
            column![
                text(group)
                    .size(settings.text_size * 0.85)
                    .font(bold)
                    .color(c.text_muted),
                row![
                    rule::vertical(3).style(style::accent_rule(c)),
                    notes.width(Fill),
                ]
                .height(Length::Shrink)
                .spacing(14),
            ]
            .spacing(6),
        );
    }

    column![
        column![heading, rule::horizontal(1).style(style::separator(c))].spacing(8),
        groups,
    ]
    .spacing(14)
    .padding(padding::bottom(40))
    .into()
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

/// The front matter fields to show as properties, as (key, label, Markdown value).
///
/// The type's fields come first, in its order and with its labels, empty ones too so they
/// can be filled in; then the note's own, if they have a value. The name (shown as the
/// title), long text fields (which are body sections), fields removed from this note, and
/// the keys the app manages are left out.
fn properties(front: &Mapping, ty: Option<&ContentType>) -> Vec<(String, String, Option<String>)> {
    let fields = ty.map(|ty| ty.fields.as_slice()).unwrap_or_default();
    let removed = front
        .get(crate::form::REMOVED_KEY)
        .map(note::value_to_text)
        .unwrap_or_default();
    let removed: Vec<&str> = removed.split(',').map(str::trim).collect();
    let mut shown = Vec::new();

    for field in fields {
        if field.key == "name" || field.kind.is_section() || removed.contains(&field.key.as_str()) {
            continue;
        }
        let value = front.get(field.key.as_str());
        let value = if field.kind == Kind::Tags && field.key != crate::form::ALIASES_KEY {
            value.and_then(|value| tag_markdown(value, &field.tag_folder()))
        } else {
            value.and_then(value_markdown)
        };
        shown.push((field.key.clone(), field.label(), value));
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
        let value = if key == "tags" {
            tag_markdown(value, crate::types::TAG_FOLDER)
        } else {
            value_markdown(value)
        };
        if let Some(value) = value {
            let label = if key.chars().any(char::is_uppercase) || key.contains(' ') {
                key.clone()
            } else {
                title_case(key)
            };
            shown.push((key.clone(), label, Some(value)));
        }
    }

    shown
}

/// The dice expressions in the field `key`, to roll from the box of fields. A link's
/// target is a note's name, never a roll. A dice field is one roll, with its advantage.
fn rolls(front: &Mapping, key: &str, ty: Option<&ContentType>) -> Vec<String> {
    let kind = ty
        .and_then(|ty| ty.fields.iter().find(|field| field.key == key))
        .map(|field| field.kind);
    let link_field = kind == Some(Kind::Link);
    // Dates, tags and links to notes are never rolls, even `2026-10-28`.
    if matches!(kind, Some(Kind::Date | Kind::Tags | Kind::Links)) {
        return Vec::new();
    }
    if kind == Some(Kind::Dice)
        && let Some(dice) = front
            .get(key)
            .and_then(|value| Dice::parse(&note::value_to_text(value)))
    {
        return vec![dice.to_string()];
    }
    match front.get(key) {
        Some(value) if !link_field && crate::form::kind_of(value) != Some(Kind::Link) => {
            // Not the names in `[[links]]` in it, either.
            let mut text = note::value_to_text(value);
            while let Some(start) = text.find("[[") {
                let end = text[start..]
                    .find("]]")
                    .map_or(text.len(), |end| start + end + 2);
                text.replace_range(start..end, " ");
            }
            Expression::find(&text)
                .iter()
                .map(ToString::to_string)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Tags as one line of Markdown, each a link to its note in `folder`, or `None` if there
/// are none.
fn tag_markdown(value: &Value, folder: &str) -> Option<String> {
    let tags = crate::tags::items(value);
    if tags.is_empty() {
        return None;
    }
    let links: Vec<String> = tags
        .iter()
        .map(|tag| {
            let file = crate::campaign::note_file_name(tag);
            let shown = tag.replace(['|', '[', ']'], " ");
            rewrite_links(&format!("[[{folder}/{file}|{}]]", shown.trim()), false)
        })
        .collect();
    Some(links.join(", "))
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
    let text = rewrite_links(&text.replace('\n', " "), false);
    Some(if block_start {
        format!("\\{text}")
    } else {
        text
    })
}

impl Embed {
    /// Reads the note that `link` (`Note` or `Note#Heading`) names, from the note at
    /// `path`, whose body is `body`.
    fn new(link: &str, body: &str, path: &Path, index: &Index) -> Embed {
        let (target, heading) = match link.split_once('#') {
            Some((target, heading)) => (target.trim(), Some(heading.trim())),
            None => (link.trim(), None),
        };
        let name = Path::new(target)
            .file_name()
            .map_or(target.to_string(), |name| {
                name.to_string_lossy().into_owned()
            });
        let title = match heading {
            Some(heading) => format!("{name} > {heading}"),
            None => name.clone(),
        };

        let content = match index.resolve(target, path) {
            // A note showing itself shows its text as it is now, saved or not.
            Some(file) if file == path => Ok(body.to_string()),
            Some(file) => std::fs::read_to_string(file)
                .map(|text| match note::split_front_matter(&text) {
                    Ok((_, body)) => body.to_string(),
                    Err(_) => text,
                })
                .map_err(|_| format!("Could not read {title}")),
            None => Err(format!(
                "There is no note called {target} yet. Click to make it."
            )),
        };
        let content = content.and_then(|text| match heading {
            Some(heading) => section(&text, heading)
                .map(str::to_string)
                .ok_or_else(|| format!("{name} has no heading called {heading}")),
            None => Ok(text),
        });

        Embed {
            title,
            target: target.to_string(),
            // Notes embedded in an embedded note show as links, so notes that embed each
            // other don't go on forever.
            content: content.map(|text| markdown::Content::parse(&rewrite_links(&text, false))),
        }
    }
}

/// Puts a picture that follows text in a paragraph on a paragraph of its own. iced's
/// Markdown widget would otherwise take the text before it as the picture's description,
/// which isn't shown once the picture is.
fn pictures_apart(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let mut breaks = Vec::new();
    let mut depth = 0;
    let mut text_before = false;
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        match event {
            Event::Start(Tag::Image { .. }) if depth == 1 && text_before => {
                breaks.push(range.start);
            }
            Event::Start(_) => depth += 1,
            Event::End(TagEnd::Image) => text_before = false,
            Event::End(_) => depth -= 1,
            Event::Text(text) if depth == 1 && !text.trim().is_empty() => text_before = true,
            Event::Code(_) if depth == 1 => text_before = true,
            _ => {}
        }
    }

    let mut out = markdown.to_string();
    for at in breaks.into_iter().rev() {
        out.insert_str(at, "\n\n");
    }
    out
}

/// The part of `text` under the heading `heading`, from it down to the next heading as
/// big or bigger, or `None` if there is no such heading. Headings are matched like
/// Obsidian: without case, and ignoring extra spaces.
fn section<'t>(text: &'t str, heading: &str) -> Option<&'t str> {
    let wanted = heading
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    fn level(line: &str) -> Option<(usize, &str)> {
        let hashes = line.len() - line.trim_start_matches('#').len();
        ((1..=6).contains(&hashes) && line[hashes..].starts_with([' ', '\t']))
            .then(|| (hashes, line[hashes..].trim().trim_end_matches('#').trim()))
    }

    let mut in_code = false;
    let mut start = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
        } else if !in_code && let Some((size, title)) = level(line.trim_end()) {
            match start {
                Some((_, wanted_size)) if size <= wanted_size => {
                    return start.map(|(from, _)| text[from..offset].trim_end());
                }
                None if title
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
                    == wanted =>
                {
                    start = Some((offset, size));
                }
                _ => {}
            }
        }
        offset += line.len();
    }
    start.map(|(from, _)| text[from..].trim_end())
}

/// Rewrites the `[[links]]` in `text` as Markdown links the reading view can click:
/// `[[Note|shown]]` becomes `[shown](<note:Note>)`, and `![[picture.png]]` a picture.
/// With `embed_notes`, `![[Note]]` becomes a picture with an `embed-note:` URL, which the
/// reading view shows as the note; otherwise embeds of notes show as links to them.
fn rewrite_links(text: &str, embed_notes: bool) -> String {
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
        let extension = Path::new(target).extension();
        if embed && is_picture(Path::new(target)) {
            out.push_str(&format!("![{}](<{EMBED}{target}>)", escape(&shown)));
        } else if embed
            && embed_notes
            && extension.is_none_or(|extension| extension.eq_ignore_ascii_case("md"))
            // Obsidian's links to blocks, `#^id`, aren't supported yet.
            && !inner.contains("#^")
        {
            let link = inner.split('|').next().unwrap_or(inner).trim();
            out.push_str(&format!("![{}](<{EMBED_NOTE}{link}>)", escape(&shown)));
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

pub fn is_picture(path: &Path) -> bool {
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
    // A full path works too, written like `C:/Pictures/map.png` or `file:///C:/...`.
    let url = match url.get(..7) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file://") => {
            let path = &url[7..];
            // `file:///C:/x` is `C:/x` on Windows.
            match path.get(2..3) {
                Some(":") if path.starts_with('/') => &path[1..],
                _ => path,
            }
        }
        _ if url.contains("://") => return None,
        _ => url,
    };
    // Joining a full path gives that path.
    let path = folder.join(decode(url));
    (path.is_file() && is_picture(&path)).then_some(path)
}

/// The text to embed the picture `file` in the note at `note` with `![[...]]`: its file
/// name, or its path in the campaign `root` if another picture has that name.
pub fn picture_link(file: &Path, note: &Path, root: &Path) -> String {
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let folder = note.parent().unwrap_or(root);
    if find_picture(&format!("{EMBED}{name}"), folder, root).as_deref() == Some(file) {
        return name.into_owned();
    }
    file.strip_prefix(root)
        .unwrap_or(file)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

pub fn markdown_style(c: Colors) -> markdown::Style {
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
#[derive(Clone)]
struct Reader<'a> {
    c: Colors,
    names: Names,
    pictures: &'a HashMap<Uri, PathBuf>,
    embeds: &'a HashMap<Uri, Embed>,
    web: &'a web::Pictures,
    code: &'a RefCell<HashMap<u64, Option<Vec<code::Line>>>>,
    /// Where the task boxes shown are in the note's text, so clicking one ticks it. Empty
    /// where they can't be ticked, like in the box of fields or another note shown here.
    tasks: &'a [usize],
    /// How many task boxes have been drawn so far, to tell which one is next.
    next_task: Cell<usize>,
}

impl<'a> Reader<'a> {
    /// The box beside a task, ticked if it is `done`. Clicking it ticks or unticks it.
    fn task_box(&self, size: iced::Pixels, done: bool) -> Element<'a, Uri> {
        let tick = text(if done { "✓" } else { "" })
            .size(size * 0.8)
            .color(self.c.accent_text)
            .center();
        let check = container(tick)
            .center(size * 1.1)
            .style(style::task_box(self.c, done));
        let check = container(check).center_y(text::LineHeight::default().to_absolute(size));

        let index = self.next_task.get();
        self.next_task.set(index + 1);
        match self.tasks.get(index) {
            Some(at) => mouse_area(check)
                .on_press(format!("{TASK}{at}"))
                .interaction(Interaction::Pointer)
                .into(),
            None => check.into(),
        }
    }

    /// A box saying why a picture isn't shown, under its description.
    fn no_picture(
        &self,
        settings: markdown::Settings,
        alt: &markdown::Text,
        why: String,
    ) -> Element<'a, Uri> {
        container(
            column![
                rich_text(alt.spans(settings.style)).size(settings.text_size),
                text(why)
                    .size(settings.text_size * 0.85)
                    .color(self.c.text_muted),
            ]
            .spacing(4),
        )
        .padding([8, 12])
        .style(style::card(self.c))
        .into()
    }

    /// A note embedded in this one: its name, which opens it, over its text.
    fn embed(&self, settings: markdown::Settings, embed: &'a Embed) -> Element<'a, Uri> {
        let link = format!("{NOTE}{}", embed.target);
        let missing = !self.names.contains(&embed.target);
        let title = rich_text([Span::new(embed.title.as_str())
            .link(link)
            .color(if missing {
                self.c.text_muted
            } else {
                self.c.accent
            })
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::DEFAULT
            })])
        .size(settings.text_size * 0.9)
        .on_link_click(|url: Uri| url);

        let body: Element<'a, Uri> = match &embed.content {
            Ok(content) if content.items().is_empty() => text("This note has no text yet.")
                .size(settings.text_size)
                .color(self.c.text_muted)
                .into(),
            // Its task boxes are ticked in that note, not here.
            Ok(content) => markdown::view_with(
                content.items(),
                settings,
                &Reader {
                    tasks: &[],
                    next_task: Cell::new(0),
                    ..self.clone()
                },
            ),
            Err(why) => text(why.clone())
                .size(settings.text_size)
                .color(self.c.text_muted)
                .into(),
        };

        row![
            rule::vertical(3).style(style::accent_rule(self.c)),
            column![title, body]
                .spacing(settings.spacing.0 * 0.75)
                .width(Fill),
        ]
        .height(Length::Shrink)
        .spacing(14)
        .into()
    }
}

impl<'a> markdown::Viewer<'a, Uri> for Reader<'a> {
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
        if url.starts_with(EMBED_NOTE)
            && let Some(embed) = self.embeds.get(url)
        {
            return self.embed(settings, embed);
        }
        if web::is_web(url) {
            return match self.web.get(url) {
                Some(web::Picture::Image(handle)) => image(handle.clone()).into(),
                Some(web::Picture::Svg(handle)) => {
                    Svg::new(handle.clone()).width(Length::Shrink).into()
                }
                Some(web::Picture::Failed(why)) => {
                    self.no_picture(settings, alt, format!("Could not load {url}: {why}"))
                }
                Some(web::Picture::Loading) | None => {
                    self.no_picture(settings, alt, format!("Loading {url}..."))
                }
            };
        }

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
            None => self.no_picture(
                settings,
                alt,
                format!("Picture not found: {}", url.trim_start_matches(EMBED)),
            ),
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
        language: Option<&'a str>,
        code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, Uri> {
        let line = |spans: Vec<Span<'a, Uri>>| -> Element<'a, Uri> {
            rich_text(spans)
                .on_link_click(Self::on_link_click)
                .font(settings.style.code_block_font)
                .size(settings.code_size)
                .into()
        };

        let parts = language.and_then(|language| {
            let mut hasher = DefaultHasher::new();
            (language, code).hash(&mut hasher);
            self.code
                .borrow_mut()
                .entry(hasher.finish())
                .or_insert_with(|| code::highlight(language, code))
                .clone()
        });
        let lines = match parts {
            Some(parts) => column(code.lines().zip(parts).map(|(text, parts)| {
                line(
                    parts
                        .into_iter()
                        .filter_map(|(range, token)| {
                            let span = Span::new(text.get(range)?);
                            Some(match token.color(self.c) {
                                Some(color) => span.color(color),
                                None => span,
                            })
                        })
                        .collect(),
                )
            })),
            None => column(
                lines
                    .iter()
                    .map(|text| line(text.spans(settings.style).to_vec())),
            ),
        };

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
                markdown::Bullet::Task { items, done } => (self.task_box(size, *done), items),
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

    fn ordered_list(
        &self,
        settings: markdown::Settings,
        start: u64,
        bullets: &'a [markdown::Bullet],
    ) -> Element<'a, Uri> {
        // Like iced's own, but with task boxes (`1. [ ] Task`) after the numbers.
        let size = settings.text_size;
        let digits = ((start + bullets.len() as u64).max(1) as f32)
            .log10()
            .ceil();
        column(bullets.iter().enumerate().map(|(i, bullet)| {
            let number = text!("{}.", i as u64 + start)
                .size(size)
                .align_x(iced::alignment::Horizontal::Right)
                .width(size * ((digits / 2.0).ceil() + 1.0));
            let mut marker = row![number].spacing(settings.spacing.0 / 2.0);
            let items = match bullet {
                markdown::Bullet::Point { items } => items,
                markdown::Bullet::Task { items, done } => {
                    marker = marker.push(self.task_box(size, *done));
                    items
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
    fn ticks_task_boxes_where_they_are() {
        let body = "Intro\n\n- [ ] Find the [[Keep]]\n  - [x] Ask Ann\n\n1. [X] Pay\n2. Rest\n";
        let at = task_boxes(body);
        assert_eq!(at.len(), 3);
        assert!(at.iter().all(|&at| body[at..].starts_with('[')));

        let ticked = toggle_task(body, at[0]).unwrap();
        assert!(ticked.contains("- [x] Find"));
        let unticked = toggle_task(&ticked, at[1]).unwrap();
        assert!(unticked.contains("  - [ ] Ask Ann"));
        assert!(toggle_task(body, at[2]).unwrap().contains("1. [ ] Pay"));
        assert_eq!(toggle_task(body, 0), None);
        assert_eq!(toggle_task(body, body.len()), None);
    }

    #[test]
    fn knows_where_each_blocks_task_boxes_are() {
        let source = "---\nname: Quests\n---\n- [ ] One\n\n> - [x] Two\n";
        let preview = Preview::new(
            source,
            None,
            Path::new("Quests.md"),
            Path::new("."),
            &Index::default(),
        );
        let tasks: Vec<&str> = preview
            .blocks
            .iter()
            .flat_map(|block| &block.tasks)
            .map(|&at| &source[at..at + 3])
            .collect();
        assert_eq!(tasks, ["[ ]", "[x]"]);
    }

    #[test]
    fn rewrites_wiki_links() {
        assert_eq!(
            rewrite_links("![[Keep]] ![[Keep#Gate|x]] ![[Keep#^abc]] ![[a.pdf]]", true),
            "![Keep](<embed-note:Keep>) ![x](<embed-note:Keep#Gate>) \
             [Keep > ^abc](<note:Keep>) [a.pdf](<note:a.pdf>)"
        );
        assert_eq!(
            rewrite_links("Ask [[Old Tom]] or [[npcs/Ann|the smith]].", false),
            "Ask [Old Tom](<note:Old Tom>) or [the smith](<note:npcs/Ann>)."
        );
        assert_eq!(
            rewrite_links("See [[Keep#Gate]] and ![[Keep]]", false),
            "See [Keep > Gate](<note:Keep>) and [Keep](<note:Keep>)"
        );
        assert_eq!(
            rewrite_links("![[maps/Town.png]]", false),
            "![maps/Town.png](<embed:maps/Town.png>)"
        );
        // Code is left alone.
        assert_eq!(rewrite_links("`[[x]]`", false), "`[[x]]`");
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
                (
                    "home".into(),
                    "Lives in".into(),
                    Some("[Town](<note:Town>)".into())
                ),
                ("mood".into(), "Mood".into(), Some("\\# grumpy".into())),
                ("alive".into(), "Alive".into(), Some("Yes".into())),
            ]
        );
    }

    #[test]
    fn titles_with_the_name_unless_the_body_has_one() {
        let path = Path::new("/campaign/npcs/Ann.md");
        let root = Path::new("/campaign");
        let named = Preview::new(
            "---\nname: Ann\n---\nHello\n",
            None,
            path,
            root,
            &Index::default(),
        );
        assert_eq!(named.title.as_deref(), Some("Ann"));

        let titled = Preview::new(
            "---\nname: Ann\n---\n# Ann the Smith\n",
            None,
            path,
            root,
            &Index::default(),
        );
        assert_eq!(titled.title, None);
    }

    #[test]
    fn puts_the_opening_text_beside_the_fields() {
        let lead = |body: &str| {
            Preview::new(
                body,
                None,
                Path::new("/c/a.md"),
                Path::new("/c"),
                &Index::default(),
            )
            .lead_length()
        };
        assert_eq!(lead("Intro.\n\nMore.\n\n## History\n\nOld.\n"), 2);
        assert_eq!(lead("## Appearance\n\nTall.\n\n## Notes\n\nx\n"), 2);
        assert_eq!(lead("Just text.\n"), 1);
    }

    #[test]
    fn splits_the_body_into_blocks() {
        let body = "Intro\nstill intro.\n\n## Notes\n\n- a\n- b\n\n```\ncode\n\nmore\n```\n\n---\n";
        let blocks: Vec<&str> = block_ranges(body).into_iter().map(|r| &body[r]).collect();
        assert_eq!(
            blocks,
            [
                "Intro\nstill intro.",
                "## Notes",
                "- a\n- b",
                "```\ncode\n\nmore\n```",
                "---"
            ]
        );

        let note = "---\nname: Ann\n---\nHello\n";
        let preview = Preview::new(
            note,
            None,
            Path::new("/c/a.md"),
            Path::new("/c"),
            &Index::default(),
        );
        assert_eq!(&note[preview.block_range(0).unwrap()], "Hello");
    }

    #[test]
    fn puts_pictures_after_text_apart() {
        assert_eq!(
            pictures_apart("Map: ![m](a.png) end"),
            "Map: \n\n![m](a.png) end"
        );
        assert_eq!(pictures_apart("![m](a.png)\n"), "![m](a.png)\n");
        // Inside a list, a blank line would end the list.
        assert_eq!(pictures_apart("- Map: ![m](a.png)"), "- Map: ![m](a.png)");
    }

    #[test]
    fn embeds_a_heading_section() {
        let text = "Intro\n\n## Gate\n\nOld.\n\n### Guards\n\nTwo.\n\n```\n## not a heading\n```\n\n## Keep\n\nTall.\n";
        assert_eq!(
            section(text, "gate"),
            Some("## Gate\n\nOld.\n\n### Guards\n\nTwo.\n\n```\n## not a heading\n```")
        );
        assert_eq!(section(text, "Keep"), Some("## Keep\n\nTall."));
        assert_eq!(section(text, "Moat"), None);
    }

    #[test]
    fn shows_another_note_inside() {
        let root =
            std::env::temp_dir().join(format!("dungeon_elements_embed_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let keep = root.join("Keep.md");
        std::fs::write(
            &keep,
            "---\nname: Keep\n---\nTall walls.\n\n## Gate\n\nOld.\n",
        )
        .unwrap();
        let index = Index::new(&root, [keep]);
        let note = root.join("Town.md");

        let preview = Preview::new(
            "![[Keep]]\n\n![[Keep#Gate]]\n\n![[Moat]]\n",
            None,
            &note,
            &root,
            &index,
        );
        let text = |url: &str| {
            let embed = &preview.embeds[url];
            embed
                .content
                .as_ref()
                .map(|content| content.items().len())
                .map_err(Clone::clone)
        };
        assert_eq!(text("embed-note:Keep"), Ok(3));
        assert_eq!(text("embed-note:Keep#Gate"), Ok(2));
        assert!(text("embed-note:Moat").is_err());
        assert_eq!(preview.embeds["embed-note:Keep#Gate"].title, "Keep > Gate");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn finds_pictures_by_full_path() {
        let picture = std::env::current_dir().unwrap().join("assets/die_icons");
        let picture = std::fs::read_dir(picture)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| is_picture(p))
            .unwrap();
        let url = picture.to_string_lossy().replace('\\', "/");
        assert_eq!(
            find_picture(&url, Path::new("/elsewhere"), Path::new("/elsewhere")),
            Some(picture.clone())
        );
        let file_url = if url.starts_with('/') {
            format!("file://{url}")
        } else {
            format!("file:///{url}")
        };
        assert_eq!(
            find_picture(&file_url, Path::new("/elsewhere"), Path::new("/elsewhere")),
            Some(picture)
        );
        assert_eq!(
            find_picture(
                "https://example.com/a.png",
                Path::new("/c"),
                Path::new("/c")
            ),
            None
        );
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
