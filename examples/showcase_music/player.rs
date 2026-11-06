//! The mini-player bar and the "Now playing" sheet. Both are small views of their own: the
//! breathing cover and the wavy progress redraw them every frame while playing, and the rest of
//! the app (in cached views) is left alone.

use gpui::{
    App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_m3::{
    ButtonGroup, ButtonGroupItem, CheckState, Checkbox, FavStar, Icon, IconButton, LinearProgress,
    M3ButtonSize, RangeSlider, RichTooltip, Shape, ShapeName, Slider, SliderSize, StopSlider,
    Switch, Theme, WavyProgress,
    theme::type_scale::{BODY_MEDIUM, HEADLINE_SMALL, LABEL_LARGE, TITLE_MEDIUM, TypeScaleExt},
};

use super::data::{ALBUMS, Covers, TRACKS, artist_of, clock_text};
use super::icons::icon;
use super::model::{Clock, Model, change, click, event};

/// The 5 equalizer bands.
const BANDS: [&str; 5] = ["60", "230", "910", "3.6k", "14k"];
const PRESETS: [(&str, [f32; 5]); 4] = [
    ("Flat", [0., 0., 0., 0., 0.]),
    ("Bass", [6., 4., 0., -1., -1.]),
    ("Vocal", [-2., 0., 3., 4., 1.]),
    ("Treble", [-2., -1., 0., 4., 6.]),
];

fn cover_art(cx: &App, album: usize, size: f32) -> Shape {
    Shape::new(ALBUMS[album].shape)
        .image(cx.global::<Covers>().0[album].clone())
        .size(px(size))
}

pub struct MiniPlayer {
    model: Entity<Model>,
    clock: Entity<Clock>,
}

impl MiniPlayer {
    pub fn new(model: Entity<Model>, clock: Entity<Clock>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        cx.observe(&clock, |_, _, cx| cx.notify()).detach();
        Self { model, clock }
    }
}

impl Render for MiniPlayer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (track, playing, volume, liked) = {
            let m = self.model.read(cx);
            (m.current, m.playing, m.volume, m.liked[m.current])
        };
        let album = TRACKS[track].album;
        let position = self.clock.read(cx).position;
        let duration = TRACKS[track].secs as f32;
        let fraction = (position / duration).clamp(0., 1.);
        let model = self.model.clone();

        let art = cover_art(cx, album, 52.);
        let art = if playing {
            art.breathe(ShapeName::Circle, 8.)
        } else {
            art
        };
        let open = {
            let model = model.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                model.update(cx, |m, cx| {
                    event("now-playing-press");
                    m.now_open = true;
                    cx.notify();
                })
            }
        };
        let info = div()
            .id("mini-info")
            .flex()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .on_click(open)
            .child(art)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(142.))
                    .overflow_hidden()
                    .child(
                        div()
                            .type_emphasized(TITLE_MEDIUM)
                            .text_size(px(14.5))
                            .truncate()
                            .child(TRACKS[track].title),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(s.on_surface_variant)
                            .truncate()
                            .child(artist_of(track)),
                    ),
            );
        let fav = FavStar::new("mini-fav", liked).on_toggle({
            let model = model.clone();
            move |on, _, cx| {
                model.update(cx, |m, cx| {
                    let t = m.current;
                    m.liked[t] = on;
                    cx.notify();
                })
            }
        });

        let controls = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                IconButton::new("mini-prev", icon("previous"))
                    .size(px(40.))
                    .tooltip("Previous")
                    .on_click(click(&model, |m, cx| m.previous(cx))),
            )
            .child(
                IconButton::new("mini-play", icon(if playing { "pause" } else { "play" }))
                    .filled()
                    .m3_size(M3ButtonSize::M)
                    .square(!playing)
                    .tooltip(if playing { "Pause" } else { "Play" })
                    .on_click(click(&model, |m, cx| {
                        m.playing = !m.playing;
                        event(if m.playing { "play" } else { "pause" });
                        cx.notify();
                    })),
            )
            .child(
                IconButton::new("mini-next", icon("next"))
                    .size(px(40.))
                    .tooltip("Next")
                    .on_click(click(&model, |m, cx| m.next(cx))),
            );

        let progress = div()
            .flex()
            .flex_1()
            .min_w(px(120.))
            .items_center()
            .gap(px(10.))
            .text_size(px(12.))
            .text_color(s.on_surface_variant)
            .child(div().w(px(32.)).child(clock_text(position)))
            .child(
                div()
                    .flex_1()
                    .h(px(12.))
                    .flex()
                    .items_center()
                    .child(if playing {
                        WavyProgress::new("mini-wave", Some(fraction)).into_any_element()
                    } else {
                        LinearProgress::new("mini-line", fraction)
                            .height(px(4.))
                            .into_any_element()
                    }),
            )
            .child(div().w(px(32.)).child(clock_text(duration)));

        let volume_icon = if volume < 1. { "volume-low" } else { "volume" };
        let tooltip = {
            let model = model.clone();
            RichTooltip::new(
                "rt-volume",
                "Drag to change the level. The equalizer lives in Now playing.",
            )
            .title("Volume")
            .action("Open equalizer", move |_, _, cx| {
                model.update(cx, |m, cx| {
                    m.now_open = true;
                    cx.notify();
                })
            })
            .build()
        };
        let volume_box = div()
            .id("mini-volume")
            .flex()
            .items_center()
            .gap(px(8.))
            .hoverable_tooltip(tooltip)
            .child(
                Icon::new(icon(volume_icon))
                    .size(px(20.))
                    .color(s.on_surface_variant),
            )
            .child(
                Slider::new("mini-volume-slider", volume, 0.0..=100.)
                    .size(SliderSize::S)
                    .width(px(96.))
                    .on_change(change(&model, |m, v: f32, cx| {
                        m.volume = v;
                        cx.notify();
                    })),
            );

        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .w_full()
            .h(px(72.))
            .pl(px(10.))
            .pr(px(20.))
            .rounded(px(36.))
            .bg(s.surface_container_high)
            .shadow(gpui_m3::Elevation::Level3.shadow(&s))
            .text_color(s.on_surface)
            .child(info)
            .child(fav)
            .child(controls)
            .child(progress)
            .child(volume_box)
    }
}

pub struct NowPlaying {
    model: Entity<Model>,
    clock: Entity<Clock>,
}

impl NowPlaying {
    pub fn new(model: Entity<Model>, clock: Entity<Clock>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        cx.observe(&clock, |_, _, cx| cx.notify()).detach();
        Self { model, clock }
    }
}

fn card(s: &gpui_m3::Scheme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(24.))
        .bg(s.surface_container)
        .p(px(16.))
}

impl Render for NowPlaying {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let m = self.model.read(cx);
        let track = m.current;
        let (playing, shuffle, repeat, liked) = (m.playing, m.shuffle, m.repeat, m.liked[track]);
        let (eq, preset, bass) = (m.eq, m.preset, m.bass);
        let (lossless, spatial, normalize) = (m.lossless, m.spatial, m.normalize);
        let album = TRACKS[track].album;
        let position = self.clock.read(cx).position;
        let duration = TRACKS[track].secs as f32;
        let model = self.model.clone();
        let clock = self.clock.clone();

        let backdrop = Shape::new(ShapeName::Cookie12)
            .size(px(214.))
            .color(s.tertiary_container);
        let backdrop = if playing {
            backdrop.spin(70.)
        } else {
            backdrop
        };
        let art = cover_art(cx, album, 176.);
        let art = if playing {
            art.breathe(ShapeName::Circle, 9.)
        } else {
            art
        };
        let stage = div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .h(px(214.))
            .child(backdrop)
            .child(div().absolute().child(art));

        let title = div()
            .flex()
            .items_center()
            .justify_between()
            .pt(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .type_emphasized(HEADLINE_SMALL)
                            .child(TRACKS[track].title),
                    )
                    .child(
                        div()
                            .type_style(BODY_MEDIUM)
                            .text_color(s.on_surface_variant)
                            .child(format!("{} · {}", artist_of(track), ALBUMS[album].name)),
                    ),
            )
            .child(FavStar::new("now-fav", liked).on_toggle({
                let model = model.clone();
                move |on, _, cx| {
                    model.update(cx, |m, cx| {
                        let t = m.current;
                        m.liked[t] = on;
                        cx.notify();
                    })
                }
            }));

        let seek = div()
            .flex()
            .flex_col()
            .pt(px(6.))
            .child(
                Slider::new("now-seek", position.min(duration), 0.0..=duration)
                    .width(px(340.))
                    .on_change(move |v, _, cx| {
                        clock.update(cx, |c, cx| {
                            c.position = v;
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .text_color(s.on_surface_variant)
                    .child(clock_text(position))
                    .child(clock_text(duration)),
            );

        let flags = [shuffle, false, playing, false, repeat];
        let transport = ButtonGroup::new(
            "now-transport",
            [
                ButtonGroupItem::icon(icon("shuffle")).tooltip("Shuffle"),
                ButtonGroupItem::icon(icon("previous")).tooltip("Previous"),
                ButtonGroupItem::icon(icon(if playing { "pause" } else { "play" }))
                    .tooltip(if playing { "Pause" } else { "Play" }),
                ButtonGroupItem::icon(icon("next")).tooltip("Next"),
                ButtonGroupItem::icon(icon("repeat")).tooltip("Repeat"),
            ],
        )
        .connected()
        .size(M3ButtonSize::S)
        .selected_many(flags)
        .on_change_many({
            let model = model.clone();
            move |new, _, cx| {
                let Some(i) = (0..5).find(|&i| new[i] != flags[i]) else {
                    return;
                };
                model.update(cx, |m, cx| {
                    match i {
                        0 => {
                            m.shuffle = !m.shuffle;
                            event(if m.shuffle {
                                "shuffle-on"
                            } else {
                                "shuffle-off"
                            });
                        }
                        1 => m.previous(cx),
                        2 => {
                            m.playing = !m.playing;
                            event(if m.playing { "play" } else { "pause" });
                        }
                        3 => m.next(cx),
                        _ => {
                            m.repeat = !m.repeat;
                            event(if m.repeat { "repeat-on" } else { "repeat-off" });
                        }
                    }
                    cx.notify();
                })
            }
        });

        let left = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(340.))
            .child(stage)
            .child(title)
            .child(seek)
            .child(div().flex().justify_center().pt(px(10.)).child(transport));

        // Equalizer: five vertical sliders, presets and the bass boost window.
        let mut bands = div().flex().justify_between().px(px(8.));
        for (i, label) in BANDS.into_iter().enumerate() {
            bands = bands.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        Slider::new(("eq-band", i), eq[i], -12.0..=12.0)
                            .vertical()
                            .centered()
                            .step(0.5)
                            .size(SliderSize::S)
                            .height(px(172.))
                            .on_change(change(&model, move |m, v: f32, cx| m.set_eq(i, v, cx))),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(s.on_surface_variant)
                            .child(label),
                    ),
            );
        }
        let presets = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                StopSlider::new("eq-presets", 5, Some(preset)).on_change(change(
                    &model,
                    |m, i: usize, cx| {
                        if let Some((_, values)) = PRESETS.get(i) {
                            m.eq = *values;
                        }
                        m.preset = i;
                        cx.notify();
                    },
                )),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(s.on_surface_variant)
                    .child(format!(
                        "Preset: {}",
                        ["Flat", "Bass", "Vocal", "Treble", "Custom"][preset.min(4)]
                    )),
            );
        let bass_window = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(13.))
                    .child(
                        div()
                            .font_weight(FontWeight(600.))
                            .child("Bass boost window"),
                    )
                    .child(
                        div()
                            .text_color(s.on_surface_variant)
                            .child(format!("{:.0}–{:.0} Hz", bass.0, bass.1)),
                    ),
            )
            .child(
                RangeSlider::new("eq-bass", bass.0, bass.1, 20.0..=400.)
                    .min_gap(40.)
                    .step(10.)
                    .width(px(488.))
                    .on_change(change(&model, |m, v: (f32, f32), cx| {
                        m.bass = v;
                        cx.notify();
                    })),
            );
        let equalizer = card(&s)
            .flex_1()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().type_emphasized(TITLE_MEDIUM).child("Equalizer"))
                    .child(
                        div()
                            .type_style(LABEL_LARGE)
                            .text_color(s.on_surface_variant)
                            .child("±12 dB"),
                    ),
            )
            .child(bands)
            .child(presets)
            .child(bass_window);

        // Audio options: switches with icons and checkboxes with a parent.
        let switch_row = |label: &'static str, hint: &'static str, switch: Switch| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight(600.))
                                .text_size(px(14.))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(s.on_surface_variant)
                                .child(hint),
                        ),
                )
                .child(switch)
        };
        let audio = card(&s)
            .flex_1()
            .gap(px(10.))
            .child(switch_row(
                "Lossless",
                "Up to 24-bit / 96 kHz",
                Switch::new("sw-lossless", lossless)
                    .thumb_icons("check", "x")
                    .on_toggle(change(&model, |m, on: bool, cx| {
                        m.lossless = on;
                        event(if on { "lossless-on" } else { "lossless-off" });
                        cx.notify();
                    })),
            ))
            .child(switch_row(
                "Spatial audio",
                "Sound that surrounds you",
                Switch::new("sw-spatial", spatial)
                    .thumb_icons(icon("waves"), "x")
                    .on_toggle(change(&model, |m, on: bool, cx| {
                        m.spatial = on;
                        event(if on { "spatial-on" } else { "spatial-off" });
                        cx.notify();
                    })),
            ));
        let checked = normalize.iter().filter(|c| **c).count();
        let parent_state = CheckState::from_count(checked, 2);
        let check_row =
            |id: &'static str, label: &'static str, checkbox: Checkbox, indent: bool| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .when(indent, |d| d.pl(px(28.)))
                    .child(checkbox)
                    .child(div().text_size(px(13.5)).child(label))
                    .id(id)
            };
        let normalize_card = card(&s)
            .flex_1()
            .gap(px(6.))
            .child(check_row(
                "chk-parent",
                "Normalize volume",
                Checkbox::new("normalize-parent", false)
                    .state(parent_state)
                    .on_change(change(&model, |m, on: bool, cx| {
                        m.normalize = [on, on];
                        event(if on {
                            "normalize-all"
                        } else {
                            "normalize-none"
                        });
                        cx.notify();
                    })),
                false,
            ))
            .child(check_row(
                "chk-loud",
                "Loudness leveling",
                Checkbox::new("normalize-0", normalize[0]).on_change(change(
                    &model,
                    |m, on: bool, cx| {
                        m.normalize[0] = on;
                        event("normalize-child");
                        cx.notify();
                    },
                )),
                true,
            ))
            .child(check_row(
                "chk-clip",
                "Prevent clipping",
                Checkbox::new("normalize-1", normalize[1]).on_change(change(
                    &model,
                    |m, on: bool, cx| {
                        m.normalize[1] = on;
                        event("normalize-child");
                        cx.notify();
                    },
                )),
                true,
            ));

        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex().gap(px(20.)).child(left).child(equalizer))
            .child(div().flex().gap(px(14.)).child(audio).child(normalize_card))
    }
}
