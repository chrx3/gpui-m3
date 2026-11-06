//! Top app bars: small, centered small, and the medium and large flexible bars that collapse.
//!
//! The bar does not read any scroll position: the host feeds [`TopAppBar::collapse`] with a
//! progress from 0 (expanded) to 1 (collapsed), usually from [`collapse_progress`].

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, canvas, div, px,
};

use crate::motion::key;
use crate::theme::{Theme, font_family, mix, type_scale};

/// Height of a collapsed bar and of the small one.
const COLLAPSED_HEIGHT: f32 = 64.;
/// Smallest scroll distance over which the container color changes (small bars never collapse).
const MIN_COLLAPSE_RANGE: f32 = 16.;
/// Height of the subtitle line.
const SUBTITLE_LINE: f32 = 20.;
/// Width assumed for the leading and trailing actions until they are measured.
const ACTION_WIDTH: f32 = 40.;

/// Size of a [`TopAppBar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TopAppBarSize {
    /// 64 px, the title on the same row as the actions.
    #[default]
    Small,
    /// Flexible bar: 112 px expanded (136 px with a subtitle), then 64 px.
    Medium,
    /// Flexible bar: 152 px expanded (168 px with a subtitle), then 64 px.
    Large,
}

impl TopAppBarSize {
    /// Height of the bar when it is not collapsed.
    pub fn expanded_height(self, has_subtitle: bool) -> Pixels {
        px(match (self, has_subtitle) {
            (TopAppBarSize::Small, _) => COLLAPSED_HEIGHT,
            (TopAppBarSize::Medium, false) => 112.,
            (TopAppBarSize::Medium, true) => 136.,
            (TopAppBarSize::Large, false) => 152.,
            (TopAppBarSize::Large, true) => 168.,
        })
    }
}

/// Scroll distance (px) a bar of `expanded_height` uses up while it collapses. A scroll view
/// that wants the bar to collapse first scrolls its own content only past this offset.
pub fn collapse_range(expanded_height: f32) -> f32 {
    (expanded_height - COLLAPSED_HEIGHT).max(MIN_COLLAPSE_RANGE)
}

/// Turns how far the content scrolled (a positive offset in px) into the `collapse` progress of a
/// bar that is `expanded_height` tall, from 0 (at the top) to 1 (fully collapsed). A small bar
/// uses the same function: its progress only drives the container color.
#[allow(clippy::manual_clamp)]
pub fn collapse_progress(scroll_offset: f32, expanded_height: f32) -> f32 {
    // `max` and `min` (not `clamp`) so that a NaN offset gives 0.
    (scroll_offset / collapse_range(expanded_height))
        .max(0.)
        .min(1.)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Height of the bar at `progress`.
fn bar_height(size: TopAppBarSize, has_subtitle: bool, progress: f32) -> f32 {
    lerp(
        f32::from(size.expanded_height(has_subtitle)),
        COLLAPSED_HEIGHT,
        progress,
    )
}

/// Where and how large the title and subtitle are at one collapse progress. The block is
/// anchored to the bottom of the bar; positions are in px.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TitleLayout {
    font_size: f32,
    line_height: f32,
    bottom: f32,
    left: f32,
    right: f32,
    subtitle_height: f32,
    subtitle_opacity: f32,
}

/// `leading` and `trailing` are the widths of the action groups (0 when there are none).
fn title_layout(
    size: TopAppBarSize,
    has_subtitle: bool,
    progress: f32,
    leading: f32,
    trailing: f32,
) -> TitleLayout {
    let flexible = size != TopAppBarSize::Small;
    let (expanded, bottom_expanded) = match size {
        TopAppBarSize::Small => (type_scale::TITLE_LARGE, 0.),
        TopAppBarSize::Medium => (type_scale::HEADLINE_SMALL, 16.),
        TopAppBarSize::Large => (type_scale::HEADLINE_MEDIUM, 24.),
    };
    let collapsed = type_scale::TITLE_LARGE;
    let subtitle = if has_subtitle { SUBTITLE_LINE } else { 0. };
    // The subtitle goes away with the collapse, except on the small bar, which has no expanded state.
    let subtitle_collapsed = if flexible { 0. } else { subtitle };

    let line_collapsed = f32::from(collapsed.line_height);
    let bottom_collapsed = (COLLAPSED_HEIGHT - line_collapsed - subtitle_collapsed) / 2.;
    let left_collapsed = if leading > 0. { leading + 16. } else { 16. };
    let right_collapsed = if trailing > 0. { trailing + 16. } else { 16. };
    let (bottom_expanded, left_expanded, right_expanded) = if flexible {
        (bottom_expanded, 16., 16.)
    } else {
        (bottom_collapsed, left_collapsed, right_collapsed)
    };
    TitleLayout {
        font_size: lerp(
            f32::from(expanded.size),
            f32::from(collapsed.size),
            progress,
        ),
        line_height: lerp(f32::from(expanded.line_height), line_collapsed, progress),
        bottom: lerp(bottom_expanded, bottom_collapsed, progress),
        left: lerp(left_expanded, left_collapsed, progress),
        right: lerp(right_expanded, right_collapsed, progress),
        subtitle_height: lerp(subtitle, subtitle_collapsed, progress),
        subtitle_opacity: if flexible {
            (1. - 2. * progress).max(0.)
        } else {
            1.
        },
    }
}

/// Top app bar: a leading navigation icon, a title and trailing actions.
///
/// The medium and large flexible bars show the title bigger on a second row (with an optional
/// subtitle) and collapse to the small size as [`TopAppBar::collapse`] goes from 0 to 1: the
/// bar gets shorter, the title shrinks and moves up beside the icons, and the container turns
/// from `surface` to `surface_container`. Connect it to a scroll view with [`collapse_progress`].
///
/// The title keeps clear of the actions by measuring them (one frame late), so they can be any
/// element; the bar takes `leading` and `trailing` as already built, usually [`super::IconButton`]s
/// of 40 px.
#[derive(IntoElement)]
pub struct TopAppBar {
    id: ElementId,
    size: TopAppBarSize,
    centered: bool,
    title: SharedString,
    subtitle: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Vec<AnyElement>,
    collapse: f32,
}

impl TopAppBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: TopAppBarSize::Small,
            centered: false,
            title: SharedString::default(),
            subtitle: None,
            leading: None,
            trailing: Vec::new(),
            collapse: 0.,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Second line under the title. It fades out while a flexible bar collapses.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Navigation icon at the start (a menu or a back button).
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Action at the end; call it once per action, in order.
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing.push(el.into_any_element());
        self
    }

    /// Centers the title of a small bar. The medium and large bars keep it at the start.
    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    pub fn size(mut self, size: TopAppBarSize) -> Self {
        self.size = size;
        self
    }

    /// Shorthand for [`TopAppBarSize::Medium`].
    pub fn medium(self) -> Self {
        self.size(TopAppBarSize::Medium)
    }

    /// Shorthand for [`TopAppBarSize::Large`].
    pub fn large(self) -> Self {
        self.size(TopAppBarSize::Large)
    }

    /// Collapse progress, 0 (expanded) to 1 (collapsed). See [`collapse_progress`].
    pub fn collapse(mut self, progress: f32) -> Self {
        self.collapse = progress;
        self
    }

    /// Height of this bar when it is not collapsed (what [`collapse_progress`] needs).
    pub fn expanded_height(&self) -> Pixels {
        self.size.expanded_height(self.subtitle.is_some())
    }
}

impl RenderOnce for TopAppBar {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let s = *Theme::of(cx);
        // `max` and `min` (not `clamp`) so that NaN gives 0.
        #[allow(clippy::manual_clamp)]
        let progress = self.collapse.max(0.).min(1.);
        let has_subtitle = self.subtitle.is_some();
        let centered = self.centered && self.size == TopAppBarSize::Small;

        // Widths of the leading and trailing groups, measured on the previous frame.
        let measured = window.use_keyed_state(key(&self.id, "actions"), cx, |_, _| [0f32; 2]);
        let known = *measured.read(cx);
        let width_of = |measured: f32, count: usize| {
            if count == 0 {
                0.
            } else if measured > 0. {
                measured
            } else {
                ACTION_WIDTH * count as f32
            }
        };
        let leading = width_of(known[0], usize::from(self.leading.is_some()));
        let trailing = width_of(known[1], self.trailing.len());

        let layout = title_layout(self.size, has_subtitle, progress, leading, trailing);
        let height = bar_height(self.size, has_subtitle, progress);
        let container = mix(s.surface_container, s.surface, progress);

        // A centered title stays in the middle, clear of the wider side.
        let (left, right) = if centered {
            let inset = layout.left.max(layout.right);
            (inset, inset)
        } else {
            (layout.left, layout.right)
        };
        let title = div()
            .h(px(layout.line_height))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .when(centered, |d| d.text_center())
            .text_size(px(layout.font_size))
            .line_height(px(layout.line_height))
            .font_weight(type_scale::TITLE_LARGE.weight)
            .text_color(s.on_surface)
            .child(self.title);
        let subtitle = self.subtitle.map(|subtitle| {
            div()
                .h(px(layout.subtitle_height))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .when(centered, |d| d.text_center())
                .opacity(layout.subtitle_opacity)
                .text_size(type_scale::LABEL_LARGE.size)
                .line_height(type_scale::LABEL_LARGE.line_height)
                .text_color(s.on_surface_variant)
                .child(subtitle)
        });
        let title_block = div()
            .absolute()
            .left(px(left))
            .right(px(right))
            .bottom(px(layout.bottom))
            .flex()
            .flex_col()
            .child(title)
            .children(subtitle);

        let group = |slot: usize, children: Vec<AnyElement>| {
            let measured = measured.clone();
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .children(children)
                .child(
                    canvas(
                        move |bounds, window, cx| {
                            let width = f32::from(bounds.size.width);
                            let changed = measured.update(cx, |widths, _| {
                                let changed = (widths[slot] - width).abs() > 0.5;
                                widths[slot] = width;
                                changed
                            });
                            if changed {
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
        };
        let actions = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(COLLAPSED_HEIGHT))
            .px(px(8.))
            .flex()
            .items_center()
            .justify_between()
            .child(group(0, self.leading.into_iter().collect()))
            .child(group(1, self.trailing));

        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w_full()
            .h(px(height))
            .overflow_hidden()
            .bg(container)
            .font_family(font_family())
            .text_color(s.on_surface)
            .child(title_block)
            .child(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_heights_follow_the_spec() {
        use TopAppBarSize::*;
        assert_eq!(Small.expanded_height(false), px(64.));
        assert_eq!(Small.expanded_height(true), px(64.));
        assert_eq!(Medium.expanded_height(false), px(112.));
        assert_eq!(Medium.expanded_height(true), px(136.));
        assert_eq!(Large.expanded_height(false), px(152.));
        assert_eq!(Large.expanded_height(true), px(168.));
    }

    #[test]
    fn progress_goes_from_zero_to_one_over_the_collapse_range() {
        // A large bar collapses over 152 - 64 = 88 px.
        assert_eq!(collapse_range(152.), 88.);
        assert_eq!(collapse_progress(0., 152.), 0.);
        assert!((collapse_progress(44., 152.) - 0.5).abs() < 1e-6);
        assert_eq!(collapse_progress(88., 152.), 1.);
        assert_eq!(collapse_progress(500., 152.), 1.);
    }

    #[test]
    fn progress_ignores_overscroll_and_bad_offsets() {
        assert_eq!(collapse_progress(-30., 112.), 0.);
        assert_eq!(collapse_progress(f32::NAN, 112.), 0.);
        assert_eq!(collapse_progress(f32::INFINITY, 112.), 1.);
    }

    #[test]
    fn a_small_bar_changes_color_over_a_short_distance() {
        assert_eq!(collapse_range(64.), MIN_COLLAPSE_RANGE);
        assert_eq!(collapse_progress(8., 64.), 0.5);
        assert_eq!(collapse_progress(100., 64.), 1.);
    }

    #[test]
    fn bar_height_shrinks_to_the_small_size() {
        use TopAppBarSize::*;
        assert_eq!(bar_height(Large, false, 0.), 152.);
        assert_eq!(bar_height(Large, false, 0.5), 108.);
        assert_eq!(bar_height(Large, false, 1.), 64.);
        assert_eq!(bar_height(Medium, true, 1.), 64.);
        assert_eq!(bar_height(Small, true, 1.), 64.);
    }

    #[test]
    fn flexible_title_shrinks_and_moves_up_beside_the_icons() {
        let at = |p| title_layout(TopAppBarSize::Large, true, p, 40., 80.);
        let (open, shut) = (at(0.), at(1.));
        // Headline medium on the second row, with room for the subtitle.
        assert_eq!((open.font_size, open.line_height), (28., 36.));
        assert_eq!((open.left, open.right, open.bottom), (16., 16., 24.));
        assert_eq!((open.subtitle_height, open.subtitle_opacity), (20., 1.));
        // Title large, centered in the 64 px bar, clear of both groups.
        assert_eq!((shut.font_size, shut.line_height), (22., 28.));
        assert_eq!((shut.left, shut.right, shut.bottom), (56., 96., 18.));
        assert_eq!((shut.subtitle_height, shut.subtitle_opacity), (0., 0.));
        let mid = at(0.5);
        assert!(mid.font_size < open.font_size && mid.font_size > shut.font_size);
        assert!(mid.bottom < open.bottom && mid.bottom > shut.bottom);
        // The subtitle is gone halfway.
        assert_eq!(mid.subtitle_opacity, 0.);
    }

    #[test]
    fn small_title_does_not_move_with_the_progress() {
        let at = |p| title_layout(TopAppBarSize::Small, true, p, 40., 0.);
        assert_eq!(at(0.), at(1.));
        // Title and subtitle are centered together in the 64 px bar.
        assert_eq!(at(0.).bottom, 8.);
        assert_eq!(at(0.).left, 56.);
        assert_eq!(at(0.).right, 16.);
    }

    #[test]
    fn title_without_actions_starts_at_the_edge() {
        let layout = title_layout(TopAppBarSize::Medium, false, 1., 0., 0.);
        assert_eq!((layout.left, layout.right), (16., 16.));
    }
}
