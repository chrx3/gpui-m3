use gpui::{
    Hsla, IntoElement, Pixels, Radians, RenderOnce, SharedString, Styled, Transformation, px, svg,
};

/// Stroke icon from the library (`icons/<name>.svg`, see [`crate::Assets::icon_names`]).
#[derive(IntoElement)]
pub struct Icon {
    name: SharedString,
    size: Pixels,
    color: Option<Hsla>,
    rotation: f32,
    scale: f32,
}

impl Icon {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            size: px(18.),
            color: None,
            rotation: 0.,
            scale: 1.,
        }
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
    fn render(self, _: &mut gpui::Window, _: &mut gpui::App) -> impl IntoElement {
        let mut el = svg()
            .path(format!("icons/{}.svg", self.name))
            .size(self.size)
            .flex_none();
        if let Some(color) = self.color {
            el = el.text_color(color);
        }
        if self.rotation != 0. || self.scale != 1. {
            el = el.with_transformation(
                Transformation::rotate(Radians(self.rotation))
                    .with_scaling(gpui::size(self.scale, self.scale)),
            );
        }
        el
    }
}
