//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

use iced::Element;
use iced::widget::space;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Dungeon Elements")
        .run()
}

#[derive(Default)]
struct App;

#[derive(Debug, Clone)]
enum Message {}

impl App {
    fn update(&mut self, message: Message) {
        match message {}
    }

    fn view(&self) -> Element<'_, Message> {
        space().into()
    }
}
