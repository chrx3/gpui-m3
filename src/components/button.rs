//! Buttons: common, toggle, icon, FAB and extended FAB.
//!
//! The M3 Expressive signature is shape: on press, the corners tighten with the
//! fast spatial spring (with bounce), and when selected a toggle button goes
//! from a pill to rounded corners.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px, relative,
};

use super::{ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, animate_on_activate, key, replay};
use crate::theme::{FONT_FAMILY, Theme, alpha};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Primary (filled with the primary color).
    Filled,
    /// Tonal (secondary container).
    #[default]
    Tonal,
    /// Outlined.
    Outlined,
    /// Text only.
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// 30 px tall.
    Small,
    /// 32 px tall, 14 px padding (the composer's model button).
    Compact,
    /// 36 px tall.
    #[default]
    Medium,
    /// 40 px tall, 20 px padding (permission and question cards).
    Comfortable,
    /// 48 px tall.
    Large,
}

impl ButtonSize {
    /// Height, horizontal padding, text size, selected radius and pressed radius.
    fn metrics(self) -> (f32, f32, f32, f32, f32) {
        match self {
            ButtonSize::Small => (30., 12., 12.5, 10., 8.),
            ButtonSize::Compact => (32., 14., 12.5, 12., 8.),
            ButtonSize::Medium => (36., 18., 13.5, 12., 8.),
            ButtonSize::Comfortable => (40., 20., 14., 14., 10.),
            ButtonSize::Large => (48., 22., 14., 16., 12.),
        }
    }
}

/// Background, content and border colors of a button.
fn palette(
    cx: &App,
    variant: ButtonVariant,
    selected: Option<bool>,
    enabled: bool,
) -> (Hsla, Hsla, Option<Hsla>) {
    let s = Theme::of(cx);
    if !enabled {
        let bg = if variant == ButtonVariant::Text {
            alpha(s.on_surface, 0.)
        } else {
            alpha(s.on_surface, 0.10)
        };
        return (bg, alpha(s.on_surface, 0.38), None);
    }
    match (variant, selected) {
        (ButtonVariant::Filled, Some(true)) | (ButtonVariant::Filled, None) => {
            (s.primary, s.on_primary, None)
        }
        (ButtonVariant::Filled, Some(false)) => {
            (s.surface_container_high, s.on_surface_variant, None)
        }
        // Tonal or text toggle: the selected background is painted by the indicator (see render).
        (ButtonVariant::Outlined, Some(true)) => (
            alpha(s.on_surface, 0.),
            s.on_secondary_container,
            Some(alpha(s.outline_variant, 0.)),
        ),
        (ButtonVariant::Outlined, Some(false)) => (
            alpha(s.on_surface, 0.),
            s.on_surface_variant,
            Some(s.outline_variant),
        ),
        (_, Some(true)) => (
            alpha(s.secondary_container, 0.),
            s.on_secondary_container,
            None,
        ),
        (_, Some(false)) => (alpha(s.on_surface, 0.), s.on_surface_variant, None),
        (ButtonVariant::Tonal, _) => (s.secondary_container, s.on_secondary_container, None),
        (ButtonVariant::Outlined, _) => (
            alpha(s.on_surface, 0.),
            s.on_surface_variant,
            Some(s.outline_variant),
        ),
        (ButtonVariant::Text, _) => (alpha(s.on_surface, 0.), s.primary, None),
    }
}

/// Button with text and, optionally, a leading icon.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    sublabel: Option<SharedString>,
    icon: Option<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    selected: Option<bool>,
    open: bool,
    disabled: bool,
    trailing: Option<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: Some(label.into()),
            sublabel: None,
            icon: None,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            selected: None,
            open: false,
            disabled: false,
            trailing: None,
            on_click: None,
        }
    }

    /// Secondary text after the label (opacity 0.75, weight 500), e.g. the effort level.
    pub fn sublabel(mut self, sublabel: impl Into<SharedString>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }

    /// The button's menu is open: corners tighten as when selected, without the toggle look.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Element after the label, such as a [`super::Badge`] ("Changes 98").
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn filled(self) -> Self {
        self.variant(ButtonVariant::Filled)
    }

    pub fn tonal(self) -> Self {
        self.variant(ButtonVariant::Tonal)
    }

    pub fn outlined(self) -> Self {
        self.variant(ButtonVariant::Outlined)
    }

    pub fn text(self) -> Self {
        self.variant(ButtonVariant::Text)
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Turns it into a toggle button: a pill when unselected, rounded corners when selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (height, pad, font, selected_radius, pressed_radius) = self.size.metrics();
        let enabled = !self.disabled;
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed && enabled;

        let radius_target = if pressed {
            pressed_radius
        } else if self.selected == Some(true) || self.open {
            selected_radius
        } else {
            height / 2.
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            radius_target,
            window,
            cx,
        ));
        let (bg, fg, border) = palette(cx, self.variant, self.selected, enabled);
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);
        let weight = if self.selected == Some(true) {
            700.
        } else {
            600.
        };
        // Selection indicator that starts at the center and stretches (as in the reference design's `.tb-btn.on::before`).
        let indicator = (self.selected.is_some()
            && self.variant != ButtonVariant::Filled
            && enabled)
            .then(|| {
                let [grow, opacity] = animate_on_activate(
                    key(&self.id, "indicator"),
                    Spring::SPATIAL_FAST,
                    self.selected == Some(true),
                    [0.35, 0.],
                    [1., 1.],
                    window,
                    cx,
                );
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .left(relative((1. - grow) / 2.))
                    .w(relative(grow.max(0.)))
                    .corners(all(radius))
                    .bg(alpha(
                        Theme::of(cx).secondary_container,
                        opacity.clamp(0., 1.),
                    ))
            });

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap_2()
            .h(px(height))
            .px(px(if self.icon.is_some() { pad - 2. } else { pad }))
            .corners(all(radius))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(font))
            .font_weight(FontWeight(weight))
            .when(self.selected == Some(true), |d| d.children(indicator))
            .child(layer);
        if let Some(border) = border {
            el = el.border_1().border_color(border);
        }
        if let Some(icon) = self.icon {
            el = el.child(Icon::new(icon).size(px(font + 4.)).color(fg));
        }
        if let Some(label) = self.label {
            el = el.child(label);
        }
        if let Some(sub) = self.sublabel {
            el = el.child(
                div()
                    .ml(px(-2.))
                    .font_weight(FontWeight(500.))
                    .opacity(0.75)
                    .child(sub),
            );
        }
        if let Some(trailing) = self.trailing {
            el = el.child(trailing);
        }
        let mut el = interactive(el, &state, enabled, true);
        if let (Some(f), true) = (self.on_click, enabled) {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconButtonVariant {
    /// No background.
    #[default]
    Standard,
    Filled,
    Tonal,
    Outlined,
}

/// Round icon button. With `hover_morph` it becomes more square on hover
/// (like the send button in the reference design).
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: SharedString,
    variant: IconButtonVariant,
    size: Pixels,
    selected: Option<bool>,
    hover_morph: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    launch: Option<u64>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            variant: IconButtonVariant::default(),
            size: px(32.),
            selected: None,
            hover_morph: false,
            disabled: false,
            tooltip: None,
            launch: None,
            on_click: None,
        }
    }

    /// Plays the "launch" feedback (send) each time `generation` changes: the button squeezes
    /// into a circle and the icon flies out the top and back in from below.
    pub fn launch(mut self, generation: u64) -> Self {
        self.launch = Some(generation);
        self
    }

    pub fn variant(mut self, variant: IconButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn filled(self) -> Self {
        self.variant(IconButtonVariant::Filled)
    }

    pub fn tonal(self) -> Self {
        self.variant(IconButtonVariant::Tonal)
    }

    pub fn outlined(self) -> Self {
        self.variant(IconButtonVariant::Outlined)
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Tooltip text shown on hover (icon-only buttons should have one).
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn hover_morph(mut self, morph: bool) -> Self {
        self.hover_morph = morph;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let enabled = !self.disabled;
        let state = use_interaction(&self.id, window, cx);
        let (hovered, pressed) = {
            let st = state.read(cx);
            (st.hovered && enabled, st.pressed && enabled)
        };
        let size = f32::from(self.size);
        let radius_target = if pressed {
            8.
        } else if self.selected == Some(true) || (self.hover_morph && hovered) {
            (size * 0.33).round()
        } else {
            size / 2.
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            radius_target,
            window,
            cx,
        ));
        // Press scale: the icon and background shrink slightly with the spring.
        let scale = animate_f32(
            key(&self.id, "scale"),
            Spring::SPATIAL_FAST,
            if pressed { 0.94 } else { 1. },
            window,
            cx,
        );

        let launch = self
            .launch
            .and_then(|generation| replay(key(&self.id, "launch"), generation, 0.6, window, cx))
            .map(launch_frame);
        let (radius, scale) = match launch {
            Some(frame) => (
                radius + (px(size / 2.) - radius) * frame.round,
                scale * frame.scale,
            ),
            None => (radius, scale),
        };

        let selected = self.selected == Some(true);
        let (bg, fg, border) = if !enabled {
            let bg = if self.variant == IconButtonVariant::Standard {
                alpha(s.on_surface, 0.)
            } else {
                alpha(s.on_surface, 0.10)
            };
            (bg, alpha(s.on_surface, 0.38), None)
        } else {
            match self.variant {
                IconButtonVariant::Standard if selected => (alpha(s.primary, 0.), s.primary, None),
                IconButtonVariant::Standard => {
                    (alpha(s.on_surface, 0.), s.on_surface_variant, None)
                }
                IconButtonVariant::Filled if self.selected == Some(false) => {
                    (s.surface_container_high, s.primary, None)
                }
                IconButtonVariant::Filled => (s.primary, s.on_primary, None),
                IconButtonVariant::Tonal if selected => {
                    (s.secondary_container, s.on_secondary_container, None)
                }
                IconButtonVariant::Tonal if self.selected == Some(false) => {
                    (s.surface_container_high, s.on_surface_variant, None)
                }
                IconButtonVariant::Tonal => (s.secondary_container, s.on_secondary_container, None),
                IconButtonVariant::Outlined if selected => (s.on_surface, s.surface, None),
                IconButtonVariant::Outlined => (
                    alpha(s.on_surface, 0.),
                    s.on_surface_variant,
                    Some(s.outline_variant),
                ),
            }
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);

        let inner = size * scale;
        let inset = (size - inner) / 2.;
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);
        let mut face = div()
            .absolute()
            .top(px(inset))
            .left(px(inset))
            .size(px(inner))
            .flex()
            .items_center()
            .justify_center()
            .corners(all(radius))
            .bg(bg)
            .child(layer)
            .child(
                div()
                    .relative()
                    .top(px(launch.map_or(0., |frame| frame.icon_y)))
                    .opacity(launch.map_or(1., |frame| frame.icon_opacity))
                    .child(
                        Icon::new(self.icon)
                            .size(px((size * 0.56).round().min(22.) * scale))
                            .color(fg),
                    ),
            );
        if let Some(border) = border {
            face = face.border_1().border_color(border);
        }
        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .size(self.size)
            .child(face);
        if let Some(text) = self.tooltip {
            el = el.tooltip(super::Tooltip::text(text));
        }
        let mut el = interactive(el, &state, enabled, true);
        if let (Some(f), true) = (self.on_click, enabled) {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

#[derive(Clone, Copy)]
struct LaunchFrame {
    scale: f32,
    /// 0 = current corners, 1 = full circle.
    round: f32,
    icon_y: f32,
    icon_opacity: f32,
}

/// Frame `t` seconds into the 0.6 s launch: squeeze to 0.82 and a circle by 30 %, then spring
/// back; the icon leaves upward (−22 px) by 45 % and re-enters from +18 px.
fn launch_frame(t: f32) -> LaunchFrame {
    let squeeze_end = 0.18;
    let (scale, round) = if t < squeeze_end {
        let k = smoothstep(t / squeeze_end);
        (1. - 0.18 * k, k)
    } else {
        let k = Spring::SPATIAL_FAST.step(t - squeeze_end);
        (0.82 + 0.18 * k, 1. - k)
    };
    let fly_end = 0.27;
    let (icon_y, icon_opacity) = if t < fly_end {
        let k = smoothstep(t / fly_end);
        (-22. * k, 1. - k)
    } else {
        let k = Spring::SPATIAL.step(t - fly_end);
        (18. * (1. - k), k.clamp(0., 1.))
    };
    LaunchFrame {
        scale,
        round: round.clamp(0., 1.),
        icon_y,
        icon_opacity,
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

/// Floating action button (56 px). It rounds out on hover and shrinks on press.
#[derive(IntoElement)]
pub struct Fab {
    id: ElementId,
    icon: SharedString,
    label: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Fab {
    pub fn new(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            label: None,
            on_click: None,
        }
    }

    /// Extended FAB (with text), like a "New conversation" button.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Fab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let (hovered, pressed) = {
            let st = state.read(cx);
            (st.hovered, st.pressed)
        };
        let extended = self.label.is_some();
        let height = if extended { 42. } else { 56. };
        let target = match (pressed, hovered, extended) {
            (true, _, _) => 12.,
            (false, true, false) => 28.,
            _ => 16.,
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            target,
            window,
            cx,
        ));
        let scale = animate_f32(
            key(&self.id, "scale"),
            Spring::SPATIAL_FAST,
            if pressed { 0.95 } else { 1. },
            window,
            cx,
        );
        let layer = state_layer(
            &self.id,
            &state,
            all(radius),
            s.on_primary_container,
            window,
            cx,
        );
        let shadow = gpui::BoxShadow {
            color: s.shadow,
            offset: gpui::point(px(0.), px(2.)),
            blur_radius: px(6.),
            spread_radius: px(0.),
        };
        let shadow_far = gpui::BoxShadow {
            color: alpha(s.shadow, 0.8),
            offset: gpui::point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(0.),
        };

        let mut face = div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .h(px(height * scale))
            .corners(all(radius))
            .bg(s.primary_container)
            .text_color(s.on_primary_container)
            .shadow(vec![shadow, shadow_far])
            .child(layer)
            .child(
                Icon::new(self.icon)
                    .size(px(if extended { 19. } else { 24. }))
                    .color(s.on_primary_container),
            );
        face = if let Some(label) = self.label {
            face.pl(px(13.))
                .pr(px(16.))
                .font_family(FONT_FAMILY)
                .text_size(px(13.5))
                .font_weight(FontWeight(600.))
                .child(label)
        } else {
            face.w(px(height * scale))
        };
        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(height))
            .when(!extended, |d| d.w(px(height)))
            .child(face);
        let mut el = interactive(el, &state, true, true);
        if let Some(f) = self.on_click {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}
