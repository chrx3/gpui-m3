//! Divider: the M3 hairline that separates content, horizontal or vertical.

use gpui::{App, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, div, px};

use crate::theme::Theme;

/// Thickness of a divider (1 dp).
const THICKNESS: f32 = 1.;
/// Inset of the indented dividers (16 dp).
const INSET: f32 = 16.;

/// How far a [`Divider`] stops short of the edges of its container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DividerInset {
    /// Edge to edge (full width).
    #[default]
    None,
    /// 16 dp at the start only (list items with a leading element).
    Start,
    /// 16 dp at both ends.
    Both,
    /// M3 "middle inset": 16 dp at both ends, the same measure as [`DividerInset::Both`].
    Middle,
}

impl DividerInset {
    /// The space left at the start and at the end, in dp.
    pub fn margins(self) -> (f32, f32) {
        match self {
            DividerInset::None => (0., 0.),
            DividerInset::Start => (INSET, 0.),
            DividerInset::Both | DividerInset::Middle => (INSET, INSET),
        }
    }
}

/// A 1 dp line in `outline_variant`. Horizontal dividers fill the width of their container;
/// vertical ones fill its height, so the container needs a definite height.
#[derive(IntoElement)]
pub struct Divider {
    vertical: bool,
    inset: DividerInset,
}

impl Divider {
    pub fn horizontal() -> Self {
        Self { vertical: false, inset: DividerInset::None }
    }

    pub fn vertical() -> Self {
        Self { vertical: true, inset: DividerInset::None }
    }

    pub fn inset(mut self, inset: DividerInset) -> Self {
        self.inset = inset;
        self
    }
}

impl RenderOnce for Divider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = Theme::of(cx).outline_variant;
        let (start, end) = self.inset.margins();
        let (start, end): (Pixels, Pixels) = (px(start), px(end));
        if self.vertical {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .h_full()
                .w(px(THICKNESS))
                .pt(start)
                .pb(end)
                .child(div().flex_1().w_full().bg(color))
        } else {
            div()
                .flex()
                .flex_none()
                .w_full()
                .h(px(THICKNESS))
                .pl(start)
                .pr(end)
                .child(div().flex_1().h_full().bg(color))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insets_follow_the_spec() {
        assert_eq!(DividerInset::None.margins(), (0., 0.));
        assert_eq!(DividerInset::Start.margins(), (16., 0.));
        assert_eq!(DividerInset::Both.margins(), (16., 16.));
        assert_eq!(DividerInset::Middle.margins(), DividerInset::Both.margins());
    }
}
