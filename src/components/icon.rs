use gpui::{
    ElementId, Hsla, IntoElement, Pixels, Radians, RenderOnce, SharedString, Styled,
    Transformation, px, svg,
};

use crate::motion::{Spring, entrance, key};

/// Stroke icon from the library (`m3/icons/<name>.svg`, see [`crate::Assets::icon_names`]).
#[derive(IntoElement)]
pub struct Icon {
    name: SharedString,
    size: Pixels,
    color: Option<Hsla>,
    rotation: f32,
    scale: f32,
    pop: Option<ElementId>,
}

impl Icon {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            size: px(18.),
            color: None,
            rotation: 0.,
            scale: 1.,
            pop: None,
        }
    }

    /// "Pops" in when it first appears: grows from 20 % while turning from -60° and fading in,
    /// with the fast spatial spring (the reference design's `m3-icon-pop`: a copy check, an unread dot). `id`
    /// must be unique within the view; give the icon a new id to pop it again.
    pub fn pop(mut self, id: impl Into<ElementId>) -> Self {
        self.pop = Some(id.into());
        self
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Visual scale (does not change the space it takes up).
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Rotation in radians.
    pub fn rotate(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let (mut rotation, mut scale, mut opacity) = (self.rotation, self.scale, 1.);
        if let Some(id) = &self.pop {
            let p = entrance(key(id, "pop"), Spring::SPATIAL_FAST, 0., window, cx);
            rotation += -std::f32::consts::FRAC_PI_3 * (1. - p);
            scale *= 0.2 + 0.8 * p;
            opacity = p.clamp(0., 1.);
        }
        let mut el = svg()
            .path(format!("{}{}.svg", crate::assets::ICON_PREFIX, self.name))
            .size(self.size)
            .flex_none();
        if let Some(color) = self.color {
            el = el.text_color(crate::theme::alpha(color, opacity));
        } else if opacity < 1. {
            el = el.opacity(opacity);
        }
        if rotation != 0. || scale != 1. {
            el = el.with_transformation(
                Transformation::rotate(Radians(rotation)).with_scaling(gpui::size(scale, scale)),
            );
        }
        el
    }
}
