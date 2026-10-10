//! Exit animations: keep an element on screen while it leaves, then unmount it.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    Styled, Window, div, px,
};

use crate::motion::{self, key};

/// How a [`Presence`] leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Exit {
    /// Fades out (scrims, menus).
    #[default]
    Fade,
    /// Fades out while sinking 8 px (the reference design's `m3-dialog-out`: banners, dialogs).
    Sink,
    /// Fades out while rising 6 px (toasts and popovers that open upward close toward the anchor).
    Rise,
}

impl Exit {
    /// Applies this exit to `el` at `progress` (1 = shown, 0 = gone).
    pub fn apply(self, el: Div, progress: f32) -> Div {
        if progress >= 1. {
            return el;
        }
        let gone = 1. - progress;
        let el = el.relative().opacity(progress.clamp(0., 1.));
        match self {
            Exit::Fade => el,
            Exit::Sink => el.top(px(gone * 8.)),
            Exit::Rise => el.top(px(gone * -6.)),
        }
    }
}

/// Like the reference design's `<Presence>`: shows its children while `show` is true and, when it turns false,
/// keeps drawing them while they animate out with the fast effects spring, then draws nothing.
/// While leaving, the children ignore the pointer.
///
/// Keep building the children while they leave (with the last data they showed). When the
/// hidden state must not take space in a layout with `gap`, use [`motion::presence`] directly
/// and add the child only while it returns `Some`.
#[derive(IntoElement)]
pub struct Presence {
    id: ElementId,
    show: bool,
    exit: Exit,
    children: Vec<AnyElement>,
}

impl Presence {
    pub fn new(id: impl Into<ElementId>, show: bool) -> Self {
        Self {
            id: id.into(),
            show,
            exit: Exit::Fade,
            children: Vec::new(),
        }
    }

    pub fn exit(mut self, exit: Exit) -> Self {
        self.exit = exit;
        self
    }
}

impl ParentElement for Presence {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Presence {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(progress) = motion::presence(key(&self.id, "presence"), self.show, window, cx)
        else {
            return div().into_any_element();
        };
        let leaving = !self.show;
        self.exit
            .apply(div(), progress)
            .children(self.children)
            .when(leaving, |d| {
                // A transparent layer on top swallows the pointer while the content leaves.
                d.child(
                    div()
                        .id(key(&self.id, "leaving"))
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .occlude(),
                )
            })
            .when(leaving, |d| d.relative())
            .into_any_element()
    }
}
