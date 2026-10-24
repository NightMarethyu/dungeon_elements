//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

use std::path::PathBuf;

use iced::keyboard;
use iced::widget::{button, column, row, text, text_editor};
use iced::{Element, Fill, Task};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Dungeon Elements")
        .run()
}

#[derive(Default)]
struct App {
    content: text_editor::Content,
    /// The file the text was last saved to, if any.
    path: Option<PathBuf>,
    /// Whether the text has changed since it was last saved.
    modified: bool,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Save,
    SaveAs,
    Saved(Result<Option<PathBuf>, String>),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                self.modified |= action.is_edit();
                self.content.perform(action);
                Task::none()
            }
            Message::Save => {
                Task::perform(save(self.path.clone(), self.content.text()), Message::Saved)
            }
            Message::SaveAs => Task::perform(save(None, self.content.text()), Message::Saved),
            Message::Saved(Ok(Some(path))) => {
                self.status = format!("Saved to {}", path.display());
                self.path = Some(path);
                self.modified = false;
                Task::none()
            }
            // The user closed the save dialog without picking a file.
            Message::Saved(Ok(None)) => Task::none(),
            Message::Saved(Err(error)) => {
                self.status = format!("Could not save: {error}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let toolbar = row![
            button("Save").on_press(Message::Save),
            button("Save As...").on_press(Message::SaveAs),
        ]
        .spacing(8);

        let editor = text_editor(&self.content)
            .placeholder("Start writing...")
            .on_action(Message::Edit)
            .key_binding(|key_press| {
                let is_ctrl_s = key_press.modifiers.command()
                    && key_press.key == keyboard::Key::Character("s".into());

                if is_ctrl_s {
                    Some(text_editor::Binding::Custom(Message::Save))
                } else {
                    text_editor::Binding::from_key_press(key_press)
                }
            })
            .height(Fill);

        let file_name = self
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map_or("Untitled".into(), |name| name.to_string_lossy());
        let marker = if self.modified {
            " (unsaved changes)"
        } else {
            ""
        };
        let status = text(format!("{file_name}{marker}  {}", self.status)).size(14);

        column![toolbar, editor, status]
            .spacing(8)
            .padding(8)
            .into()
    }
}

/// Writes `text` to `path`, asking the user where to save first if there is no path yet.
///
/// Returns `Ok(None)` if the user cancels the save dialog.
async fn save(path: Option<PathBuf>, text: String) -> Result<Option<PathBuf>, String> {
    let path = match path {
        Some(path) => path,
        None => {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("Save as Markdown")
                .add_filter("Markdown", &["md"])
                .set_file_name("untitled.md")
                .save_file()
                .await;

            match picked {
                Some(handle) => with_md_extension(handle.path().to_path_buf()),
                None => return Ok(None),
            }
        }
    };

    std::fs::write(&path, text).map_err(|error| error.to_string())?;

    Ok(Some(path))
}

/// Appends `.md` to `path` unless it already ends in it, so files are always Markdown.
fn with_md_extension(mut path: PathBuf) -> PathBuf {
    let is_md = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));

    if !is_md {
        path.as_mut_os_string().push(".md");
    }

    path
}
