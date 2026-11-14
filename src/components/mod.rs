mod button;
mod button_group;
mod carousel;
mod chip;
mod choice;
mod code;
mod code_buffer;
mod code_editor;
mod color;
mod date_picker;
mod diff;
mod disclosure;
mod divider;
mod fab_menu;
mod feedback;
mod icon;
mod image_cropper;
#[cfg(feature = "json")]
mod json;
#[cfg(feature = "json")]
pub use json::JsonView;
mod keys;
mod lists;
mod markdown;
#[cfg(feature = "mermaid")]
mod mermaid;
#[cfg(feature = "mermaid")]
pub use mermaid::MermaidDiagram;
mod media;
mod nav;
mod overlays;
mod palette;
mod photo;
mod presence;
mod reorder;
mod resize;
mod rich_tooltip;
mod search;
mod segmented;
mod selectable;
mod setting_row;
mod sheets;
mod slider;
mod status;
mod suggest;
mod surfaces;
mod switch;
mod text_area;
mod text_field;
mod theme_motion;
mod time_picker;
mod toolbar;
mod top_app_bar;
mod visual;

pub use button::*;
pub use button_group::*;
pub use carousel::{Carousel, CarouselItem, CarouselLayout};
pub use chip::*;
pub use choice::*;
pub use code::*;
pub use code_buffer::*;
pub use code_editor::*;
pub use color::*;
pub use date_picker::*;
pub use diff::*;
pub use disclosure::*;
pub use divider::*;
pub use fab_menu::*;
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
pub use rich_tooltip::*;
pub use search::*;
pub use segmented::*;
pub use selectable::*;
pub use setting_row::*;
pub use sheets::*;
pub use slider::*;
pub use status::*;
pub use suggest::*;
pub use surfaces::*;
pub use switch::*;
pub use text_area::*;
pub use text_field::*;
pub use theme_motion::*;
pub use time_picker::*;
pub use toolbar::*;
pub use top_app_bar::*;
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
