//! "Sliders" section.

use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};
use gpui_m3::{RangeSlider, Segment, SegmentedButtons, Slider, SliderSize, StopSlider, Theme};

use super::{column, demo, page, row};

const SIZES: [(&str, SliderSize); 6] = [
    ("Standard", SliderSize::Standard),
    ("XS", SliderSize::Xs),
    ("S", SliderSize::S),
    ("M", SliderSize::M),
    ("L", SliderSize::L),
    ("XL", SliderSize::Xl),
];

/// Demos of the slider variants, in their own view (they keep their own state).
pub struct SliderDemos {
    /// Sizes, range, vertical or options (M3_PAGE=<n> opens one directly).
    page: usize,
    sizes: [f32; 6],
    range: (f32, f32),
    range_gap: (f32, f32),
    range_xl: (f32, f32),
    vertical: f32,
    vertical_range: (f32, f32),
    vertical_stop: Option<usize>,
    centered: f32,
    centered_mid: f32,
    indicator: f32,
    indicator_range: (f32, f32),
    icon: [f32; 2],
    ticks: [f32; 2],
}

impl SliderDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            page: std::env::var("M3_PAGE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(3),
            sizes: [40., 25., 50., 65., 35., 75.],
            range: (25., 70.),
            range_gap: (30., 60.),
            range_xl: (20., 80.),
            vertical: 60.,
            vertical_range: (20., 75.),
            vertical_stop: Some(2),
            centered: 30.,
            centered_mid: 7.,
            indicator: 42.,
            indicator_range: (30., 65.),
            icon: [55., 80.],
            ticks: [50., 3.],
        }
    }

    fn set<T: 'static>(
        &self,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, T) + 'static,
    ) -> impl Fn(T, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |value, _, cx| {
            this.update(cx, |d, cx| {
                apply(d, value);
                cx.notify();
            })
            .ok();
        }
    }

    fn sizes_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut sliders = column().gap(px(18.));
        for (i, (name, size)) in SIZES.into_iter().enumerate() {
            sliders = sliders.child(
                row()
                    .gap(px(16.))
                    .child(
                        div()
                            .w(px(70.))
                            .text_color(s.on_surface_variant)
                            .child(name),
                    )
                    .child(
                        Slider::new(("size", i), self.sizes[i], 0.0..=100.)
                            .size(size)
                            .width(px(320.))
                            .on_change(self.set(cx, move |d, v| d.sizes[i] = v)),
                    ),
            );
        }
        let icons = column()
            .gap(px(18.))
            .child(
                Slider::new(("icon", 0usize), self.icon[0], 0.0..=100.)
                    .size(SliderSize::L)
                    .icon("star")
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.icon[0] = v)),
            )
            .child(
                Slider::new(("icon", 1usize), self.icon[1], 0.0..=100.)
                    .size(SliderSize::Xl)
                    .icon("bolt")
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.icon[1] = v)),
            );
        page()
            .child(demo(
                "Sizes",
                "Standard is the original look; XS to XL are the Expressive tracks (16, 24, 40, 56 and 96 dp).",
                sliders,
            ))
            .child(demo(
                "Icon",
                "L and XL tracks can carry an icon at the start; its color flips with the track and it hides under the handle.",
                icons,
            ))
    }

    fn range_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let text = |(low, high): (f32, f32)| {
            div()
                .text_color(s.on_surface_variant)
                .child(format!("{low:.0} to {high:.0}"))
        };
        let basic = row()
            .gap(px(16.))
            .child(
                RangeSlider::new("range", self.range.0, self.range.1, 0.0..=100.)
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.range = v)),
            )
            .child(text(self.range));
        let stepped = row()
            .gap(px(16.))
            .child(
                RangeSlider::new("range-gap", self.range_gap.0, self.range_gap.1, 0.0..=100.)
                    .step(5.)
                    .min_gap(10.)
                    .size(SliderSize::M)
                    .ticks(20)
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.range_gap = v)),
            )
            .child(text(self.range_gap));
        let large = row()
            .gap(px(16.))
            .child(
                RangeSlider::new("range-xl", self.range_xl.0, self.range_xl.1, 0.0..=100.)
                    .size(SliderSize::Xl)
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.range_xl = v)),
            )
            .child(text(self.range_xl));
        let disabled = RangeSlider::new("range-off", 30., 60., 0.0..=100.)
            .width(px(320.))
            .disabled(true);
        page()
            .child(demo(
                "Range slider",
                "Two handles; a click on the track moves the nearest one.",
                basic,
            ))
            .child(demo(
                "Step, minimum gap and ticks",
                "Steps of 5 with at least 10 between the handles.",
                stepped,
            ))
            .child(demo("Large", "The same look in the XL size.", large))
            .child(demo("Disabled", "No handles to drag.", disabled))
    }

    fn vertical_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let label = |text: String| div().text_color(s.on_surface_variant).child(text);
        let item = |body: gpui::AnyElement, text: String| {
            column()
                .items_center()
                .gap(px(10.))
                .child(body)
                .child(label(text))
        };
        let sliders = row()
            .gap(px(40.))
            .items_end()
            .child(item(
                Slider::new("v-slider", self.vertical, 0.0..=100.)
                    .vertical()
                    .height(px(200.))
                    .on_change(self.set(cx, |d, v| d.vertical = v))
                    .into_any_element(),
                format!("{:.0}", self.vertical),
            ))
            .child(item(
                Slider::new("v-slider-m", self.vertical, 0.0..=100.)
                    .vertical()
                    .size(SliderSize::M)
                    .height(px(200.))
                    .on_change(self.set(cx, |d, v| d.vertical = v))
                    .into_any_element(),
                "M".into(),
            ))
            .child(item(
                RangeSlider::new(
                    "v-range",
                    self.vertical_range.0,
                    self.vertical_range.1,
                    0.0..=100.,
                )
                .vertical()
                .height(px(200.))
                .on_change(self.set(cx, |d, v| d.vertical_range = v))
                .into_any_element(),
                format!(
                    "{:.0} to {:.0}",
                    self.vertical_range.0, self.vertical_range.1
                ),
            ))
            .child(item(
                StopSlider::new("v-stop", 5, self.vertical_stop)
                    .vertical()
                    .on_change(self.set(cx, |d, i: usize| d.vertical_stop = Some(i)))
                    .into_any_element(),
                self.vertical_stop
                    .map_or("-".into(), |i| format!("stop {i}")),
            ))
            .child(item(
                Slider::new("v-off", 40., 0.0..=100.)
                    .vertical()
                    .height(px(200.))
                    .disabled(true)
                    .into_any_element(),
                "off".into(),
            ));
        page().child(demo(
            "Vertical",
            "The bottom is the minimum. Slider, range slider and stepped slider; Up and Down move a focused handle.",
            sliders,
        ))
    }

    fn options_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let value = |v: f32| {
            div()
                .text_color(s.on_surface_variant)
                .child(format!("{v:.0}"))
        };
        let centered = column()
            .gap(px(18.))
            .child(
                row()
                    .gap(px(16.))
                    .child(
                        Slider::new("centered", self.centered, -100.0..=100.)
                            .centered()
                            .width(px(320.))
                            .on_change(self.set(cx, |d, v| d.centered = v)),
                    )
                    .child(value(self.centered)),
            )
            .child(
                row()
                    .gap(px(16.))
                    .child(
                        Slider::new("centered-mid", self.centered_mid, 0.0..=10.)
                            .centered()
                            .size(SliderSize::S)
                            .step(1.)
                            .width(px(320.))
                            .on_change(self.set(cx, |d, v| d.centered_mid = v)),
                    )
                    .child(value(self.centered_mid)),
            );
        let indicator = column()
            .gap(px(36.))
            .pt(px(34.))
            .child(
                Slider::new("indicator", self.indicator, 0.0..=100.)
                    .value_indicator(true)
                    .size(SliderSize::S)
                    .format(|v| format!("{v:.0}%"))
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.indicator = v)),
            )
            .child(
                RangeSlider::new(
                    "indicator-range",
                    self.indicator_range.0,
                    self.indicator_range.1,
                    0.0..=100.,
                )
                .value_indicator(true)
                .width(px(320.))
                .on_change(self.set(cx, |d, v| d.indicator_range = v)),
            );
        let ticks = column()
            .gap(px(18.))
            .child(
                Slider::new("ticks", self.ticks[0], 0.0..=100.)
                    .ticks(10)
                    .size(SliderSize::M)
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.ticks[0] = v)),
            )
            .child(
                Slider::new("ticks-step", self.ticks[1], 0.0..=5.)
                    .ticks(5)
                    .step(1.)
                    .size(SliderSize::L)
                    .width(px(320.))
                    .on_change(self.set(cx, |d, v| d.ticks[1] = v)),
            );
        let off = column()
            .gap(px(12.))
            .child(Slider::new("off", 40., 0.0..=100.).disabled(true))
            .child(
                Slider::new("off-m", 60., 0.0..=100.)
                    .size(SliderSize::M)
                    .ticks(5)
                    .disabled(true),
            );
        page()
            .child(demo(
                "Centered",
                "The active track starts at the middle of the range (zero here).",
                centered,
            ))
            .child(demo(
                "Value indicator",
                "A bubble above the handle while it is pressed, dragged or focused with the keyboard.",
                indicator,
            ))
            .child(demo(
                "Ticks",
                "Stop indicators on the track: continuous in the first, snapped with a step in the second.",
                ticks,
            ))
            .child(demo("Disabled", "No input, muted colors.", off))
    }
}

impl Render for SliderDemos {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let pages = SegmentedButtons::new(
            "slider-page",
            ["Sizes", "Range", "Vertical", "Options"].map(Segment::label),
            self.page,
        )
        .on_change(move |i, _, cx| {
            this.update(cx, |d, cx| {
                d.page = i;
                cx.notify();
            })
            .ok();
        });
        let content = match self.page {
            0 => self.sizes_page(cx).into_any_element(),
            1 => self.range_page(cx).into_any_element(),
            2 => self.vertical_page(cx).into_any_element(),
            _ => self.options_page(cx).into_any_element(),
        };
        page().child(pages).child(content)
    }
}
