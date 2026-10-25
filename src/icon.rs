//! The app icon: the colored logo, shown on the window and in the taskbar.
//!
//! The Windows executable gets the same logo from `assets/dungeon_elements.ico` (see `build.rs`).

use iced::window::icon::{self, Icon};
use resvg::{tiny_skia, usvg};

/// The logo in color. Unlike the menu bar's logo, it isn't recolored to match the theme.
const COLOR_LOGO: &str = include_str!("../reference/dungeon_elements_colors.svg");

/// The size the icon is drawn at; the system scales it down where it needs to.
const SIZE: u32 = 256;

/// The window icon, or `None` if the logo can't be drawn (the window then has the default icon).
pub fn window_icon() -> Option<Icon> {
    let rgba = render(COLOR_LOGO, SIZE)?;
    icon::from_rgba(rgba, SIZE, SIZE).ok()
}

/// Draws `svg` in a `size` by `size` square and returns its RGBA pixels.
fn render(svg: &str, size: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;

    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // tiny-skia stores premultiplied alpha; window icons want it straight.
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_the_logo() {
        let rgba = render(COLOR_LOGO, 32).unwrap();
        assert_eq!(rgba.len(), 32 * 32 * 4);
        // Not blank: some pixels are drawn, and the corners are see-through.
        assert!(rgba.chunks(4).any(|pixel| pixel[3] == 255));
        assert_eq!(rgba[3], 0);
        assert!(window_icon().is_some());
    }
}
