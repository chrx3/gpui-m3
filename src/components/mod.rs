mod button;
mod chip;
mod choice;
mod code;
mod code_buffer;
mod code_editor;
mod color;
mod diff;
mod disclosure;
mod feedback;
mod icon;
mod image_cropper;
mod keys;
mod lists;
mod markdown;
mod media;
mod nav;
mod overlays;
mod palette;
mod photo;
mod presence;
mod reorder;
mod resize;
mod segmented;
mod setting_row;
mod slider;
mod suggest;
mod surfaces;
mod switch;
mod text_area;
mod text_field;
mod theme_motion;
mod visual;

pub use button::*;
pub use chip::*;
pub use choice::*;
pub use code::*;
pub use code_buffer::*;
pub use code_editor::*;
pub use color::*;
pub use diff::*;
pub use disclosure::*;
pub use feedback::*;
pub use icon::*;
pub use image_cropper::*;
pub use keys::*;
pub use lists::*;
pub use markdown::*;
pub use media::*;
pub use nav::*;
pub use overlays::*;
pub use palette::*;
pub use photo::PhotoSource;
pub use presence::*;
pub use reorder::*;
pub use resize::*;
pub use segmented::*;
pub use setting_row::*;
pub use slider::*;
pub use suggest::*;
pub use surfaces::*;
pub use switch::*;
pub use text_area::*;
pub use text_field::*;
pub use theme_motion::*;
pub use visual::*;

use std::rc::Rc;

use gpui::{App, ClickEvent, Corners, ElementId, Pixels, Window};

/// Click callback shared by the components.
pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Value-change callback (selected index, a switch's state…).
pub type ChangeHandler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// All four corners with the same radius.
pub(crate) fn all(r: Pixels) -> Corners<Pixels> {
    Corners {
        top_left: r,
        top_right: r,
        bottom_right: r,
        bottom_left: r,
    }
}

/// Corners from an array [top-left, top-right, bottom-right, bottom-left].
pub(crate) fn corners(r: [f32; 4]) -> Corners<Pixels> {
    Corners {
        top_left: gpui::px(r[0].max(0.)),
        top_right: gpui::px(r[1].max(0.)),
        bottom_right: gpui::px(r[2].max(0.)),
        bottom_left: gpui::px(r[3].max(0.)),
    }
}

/// Id of a component part (e.g. the two halves of a split button).
pub(crate) fn child_id(id: &ElementId, part: &'static str) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), part.into())
}

/// Applies per-corner radii to an element.
pub(crate) trait RoundedExt: gpui::Styled + Sized {
    fn corners(self, c: Corners<Pixels>) -> Self {
        self.rounded_tl(c.top_left)
            .rounded_tr(c.top_right)
            .rounded_br(c.bottom_right)
            .rounded_bl(c.bottom_left)
    }
}

impl<T: gpui::Styled> RoundedExt for T {}
