//! Right-click menus.
//!
//! iced has no context menus, so [`area`] notices right-clicks on what it wraps and says
//! where the mouse was, and [`view`] draws the menu there, as a layer over the whole
//! window, the way the menu bar's dropdowns are drawn.

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::widget::{mouse_area, opaque, pin, space, stack};
use iced::{Element, Event, Fill, Length, Point, Rectangle, Size, Vector};

use crate::menu::{self, Entry};
use crate::theme::Colors;

const WIDTH: f32 = 250.0;

/// Sends `on_open` with the mouse's place in the window when `content` is right-clicked.
///
/// Areas can be inside each other: the innermost one under the mouse opens its menu.
pub fn area<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_open: impl Fn(Point) -> Message + 'a,
) -> Area<'a, Message> {
    Area {
        content: content.into(),
        on_open: Box::new(on_open),
        click_first: false,
    }
}

pub struct Area<'a, Message> {
    content: Element<'a, Message>,
    on_open: Box<dyn Fn(Point) -> Message + 'a>,
    click_first: bool,
}

impl<Message> Area<'_, Message> {
    /// Whether to click the content where it was right-clicked, as a left click would,
    /// before the menu opens. In a text editor, that puts the cursor there, so the menu
    /// is about the word that was clicked. Leave it off to keep the selected text.
    pub fn click_first(mut self, click: bool) -> Self {
        self.click_first = click;
        self
    }
}

/// How far the area is from where it would be without scrolling, kept from the last time
/// the window was laid out: the mouse's place given to widgets inside a scrollable is
/// where it is on the scrolled content, not in the window.
#[derive(Default)]
struct State {
    translation: Vector,
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for Area<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        // An area inside this one opened its menu already.
        if shell.is_event_captured() {
            return;
        }
        let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) = event else {
            return;
        };
        let Some(position) = cursor.position_over(layout.bounds()) else {
            return;
        };

        if self.click_first {
            for event in [
                mouse::Event::ButtonPressed(mouse::Button::Left),
                mouse::Event::ButtonReleased(mouse::Button::Left),
            ] {
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    &Event::Mouse(event),
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
        }

        let translation = tree.state.downcast_ref::<State>().translation;
        shell.publish((self.on_open)(position + translation));
        shell.capture_event();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        // Every widget is asked for its overlay before events reach it, with the offset
        // scrollables around it add, which turns the mouse's place back into the window's.
        tree.state.downcast_mut::<State>().translation = translation;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<Area<'a, Message>> for Element<'a, Message> {
    fn from(area: Area<'a, Message>) -> Self {
        Element::new(area)
    }
}

/// The menu at `position`, as a layer over the whole window of size `window`.
///
/// It opens down and to the right of the mouse, or up and to the left where there is no
/// room. Clicking (or right-clicking) anywhere outside it sends `on_close`.
pub fn view<'a, Message: Clone + 'a>(
    c: Colors,
    position: Point,
    window: Size,
    entries: Vec<Entry<Message>>,
    on_close: Message,
) -> Element<'a, Message> {
    let height = menu::panel_height(&entries);
    let x = if position.x + WIDTH > window.width {
        (position.x - WIDTH).max(0.0)
    } else {
        position.x
    };
    let y = if position.y + height > window.height {
        (position.y - height).max(0.0)
    } else {
        position.y
    };

    let catcher = mouse_area(space().width(Fill).height(Fill))
        .on_press(on_close.clone())
        .on_right_press(on_close);

    stack![
        catcher,
        pin(opaque(menu::panel(c, entries, WIDTH))).x(x).y(y),
    ]
    .into()
}
