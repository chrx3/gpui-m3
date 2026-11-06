//! A music player built only from gpui-m3 components: the hero of the 0.2.0 launch video.
//!
//! `cargo run --example showcase_music`
//!
//! Environment (all optional):
//! - `M3_SIZE=<w>x<h>` (default 1280x720), `M3_POS=<x>,<y>` window position, `M3_MODE=light|dark`,
//!   `M3_SEED=<n>` first album, `M3_VARIANT=0..8`, `M3_CONTRAST=-1..1`;
//!   `M3_OPEN=now|queue|search|fab|playlist|sleep|library` opens something at the start.
//! - `SHOWCASE_PAUSED=1` starts paused (the idle cost is then close to zero).
//! - `SHOWCASE_FRAMELESS=1` opens the window without the OS title bar.
//! - `SHOWCASE_DEMO=1` runs a scripted tour of about a minute and prints `SHOWCASE_DEMO: done`.
//! - `SHOWCASE_CLIP=repaint|carousel|sheet|toggles|search|queue|fab|palettes|reveal` runs one
//!   short scripted clip and prints `SHOWCASE_CLIP: done`. Both print `SHOWCASE_EVENT t=<ms> <name>`
//!   lines, with t counted from the first frame drawn. `SHOWCASE_EXIT=1` quits after a script.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyView, App, AppContext as _, Application, Bounds, Context, Entity, IntoElement,
    ParentElement, Pixels, Point, Render, StyleRefinement, Styled, TitlebarOptions, Window,
    WindowBounds, WindowOptions, canvas, div, point, prelude::FluentBuilder as _, px, size,
};
use gpui_m3::{
    Badge, Button, Dialog, FabMenu, Icon, IconButton, MotionSettings, RailItem, SchemeVariant,
    Shape, ShapeName, TextField, Theme, ThemeReveal, TimePicker, Toast, Toolbar, ToolbarItem,
    motion::{animate_f32, key, presence},
    theme_reveal_at,
};

#[path = "showcase_music/data.rs"]
mod data;
#[path = "showcase_music/icons.rs"]
mod icons;
#[path = "showcase_music/model.rs"]
mod model;
#[path = "showcase_music/page.rs"]
mod page;
#[path = "showcase_music/player.rs"]
mod player;
#[path = "showcase_music/queue.rs"]
mod queue;
#[path = "showcase_music/tour.rs"]
mod tour;

use data::{ALBUMS, Covers, TRACKS, draw_cover};
use icons::{ShowcaseAssets, icon};
use model::{Clock, Model, START_POSITION, click, event, mark_start};
use page::Page;
use player::{MiniPlayer, NowPlaying};
use queue::QueuePanel;

/// Height of the band under the page that holds the mini-player.
const BAND: f32 = 96.;

/// Switches the mode with the circular reveal growing from `origin`.
pub fn set_dark(
    model: &Entity<Model>,
    dark: bool,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let scheme = model.update(cx, |m, cx| {
        m.dark = dark;
        cx.notify();
        m.scheme()
    });
    event(if dark {
        "reveal-to-dark"
    } else {
        "reveal-to-light"
    });
    theme_reveal_at(scheme, dark, origin, window, cx);
}

pub struct Shell {
    model: Entity<Model>,
    clock: Entity<Clock>,
    page: Entity<Page>,
    queue: Entity<QueuePanel>,
    mini: Entity<MiniPlayer>,
    now: Entity<NowPlaying>,
    name_field: Entity<TextField>,
    /// Center of the theme button, where scripted reveals start.
    toggle_origin: Rc<Cell<Point<Pixels>>>,
}

impl Shell {
    fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let clock = cx.new(|_| Clock {
            position: START_POSITION,
        });
        let page = cx.new(|cx| Page::new(model.clone(), cx));
        let queue = cx.new(|cx| QueuePanel::new(model.clone(), cx));
        let mini = cx.new(|cx| MiniPlayer::new(model.clone(), clock.clone(), cx));
        let now = cx.new(|cx| NowPlaying::new(model.clone(), clock.clone(), cx));
        let name_field = cx.new(|cx| {
            TextField::new(cx)
                .outlined()
                .label("Playlist name")
                .max_length(30)
                .supporting_text("Something you will recognize later")
        });
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        let shell = Self {
            model,
            clock,
            page,
            queue,
            mini,
            now,
            name_field,
            toggle_origin: Rc::new(Cell::new(point(px(40.), px(560.)))),
        };
        shell.start_playback(cx);
        tour::start(window, cx);
        shell
    }

    /// Moves the clock while a track plays; at the end the next track starts.
    fn start_playback(&self, cx: &mut Context<Self>) {
        let (model, clock) = (self.model.clone(), self.clock.clone());
        cx.spawn(async move |_, cx| {
            let mut last = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let now = std::time::Instant::now();
                let dt = (now - last).as_secs_f32();
                last = now;
                let finished = cx
                    .update(|cx| {
                        let m = model.read(cx);
                        if !m.playing {
                            return false;
                        }
                        let duration = TRACKS[m.current].secs as f32;
                        let position = clock.read(cx).position + dt;
                        let finished = position >= duration;
                        clock.update(cx, |c, cx| {
                            c.position = if finished { 0. } else { position };
                            cx.notify();
                        });
                        finished
                    })
                    .unwrap_or(false);
                if finished {
                    cx.update(|cx| model.update(cx, |m, cx| m.next(cx))).ok();
                }
            }
        })
        .detach();
    }

    fn rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (nav, dark, fab_open) = {
            let m = self.model.read(cx);
            (m.nav, m.dark, m.fab_open)
        };
        let search_open = self.page.read(cx).search.read(cx).is_expanded();
        let model = self.model.clone();
        let page = self.page.clone();

        let logo = div().pt(px(20.)).pb(px(12.)).child(
            Shape::new(ShapeName::Cookie9)
                .size(px(44.))
                .color(s.primary)
                .child(Icon::new(icon("music")).size(px(22.)).color(s.on_primary)),
        );
        let home = RailItem::new("rail-home", icon("home"), "Home")
            .selected(nav == 0 && !search_open)
            .on_click(click(&model, |m, cx| {
                m.nav = 0;
                cx.notify();
            }));
        let search = RailItem::new("rail-search", "search", "Search")
            .selected(search_open)
            .on_click({
                let model = model.clone();
                move |_, window, cx| {
                    model.update(cx, |m, cx| {
                        m.nav = 0;
                        cx.notify();
                    });
                    page.update(cx, |p, cx| p.open_search(window, cx));
                }
            });
        let library = div()
            .relative()
            .child(
                RailItem::new("rail-library", icon("library"), "Library")
                    .selected(nav == 2)
                    .on_click(click(&model, |m, cx| {
                        m.nav = 2;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(41.))
                    .child(Badge::new("3").strong(gpui_m3::Tone::Primary)),
            );

        let origin = self.toggle_origin.clone();
        let toggle = div()
            .relative()
            .child(
                IconButton::new("rail-theme", if dark { "sun" } else { "moon" })
                    .tonal()
                    .size(px(44.))
                    .tooltip(if dark { "Light theme" } else { "Dark theme" })
                    .on_click({
                        let model = model.clone();
                        move |_, window, cx| {
                            let origin = window.mouse_position();
                            set_dark(&model, !dark, origin, window, cx);
                        }
                    }),
            )
            .child(
                canvas(
                    move |bounds, _, _| origin.set(bounds.center()),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        let fab = FabMenu::new("rail-fab")
            .color(gpui_m3::FabMenuColor::Tertiary)
            .align_start()
            .open(fab_open)
            .item(
                icon("playlist-add"),
                "New playlist",
                click(&model, |m, cx| {
                    m.playlist_open = true;
                    cx.notify();
                }),
            )
            .item(
                "import",
                "Import",
                click(&model, |m, cx| {
                    event("fab-import");
                    m.say("Imported 12 songs from Downloads", cx);
                }),
            )
            .item(
                icon("radio"),
                "Radio",
                click(&model, |m, cx| {
                    event("fab-radio");
                    m.play_track(12, cx);
                    m.say("Radio started from Petal Pulse", cx);
                }),
            )
            .on_toggle(click_toggle(&model));

        div()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .w(px(80.))
            .h_full()
            .child(logo)
            .child(home)
            .child(search)
            .child(library)
            .child(div().flex_1())
            .child(toggle)
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w_full()
                    .h(px(BAND))
                    .child(div().absolute().left(px(12.)).top(px(16.)).child(fab)),
            )
    }

    fn bottom_band(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (queue_open, sleep) = {
            let m = self.model.read(cx);
            (m.queue_open, m.sleep.is_some())
        };
        let model = self.model.clone();
        let toolbar = Toolbar::floating()
            .item(
                ToolbarItem::new("tool-queue", icon("queue"))
                    .selected(queue_open)
                    .tooltip("Queue")
                    .on_click(click(&model, |m, cx| {
                        m.queue_open = !m.queue_open;
                        event(if m.queue_open {
                            "queue-open"
                        } else {
                            "queue-close"
                        });
                        cx.notify();
                    })),
            )
            .item(
                ToolbarItem::new("tool-eq", icon("equalizer"))
                    .tooltip("Equalizer")
                    .on_click(click(&model, |m, cx| {
                        m.now_open = true;
                        cx.notify();
                    })),
            )
            .item(
                ToolbarItem::new("tool-sleep", icon("timer"))
                    .selected(sleep)
                    .tooltip("Sleep timer")
                    .on_click(click(&model, |m, cx| {
                        m.sleep_open = true;
                        cx.notify();
                    })),
            );
        div()
            .flex()
            .flex_none()
            .items_start()
            .gap(px(12.))
            .w_full()
            .h(px(BAND))
            .pt(px(8.))
            .pr(px(16.))
            .child(div().flex_1().min_w_0().child(self.mini.clone()))
            .child(div().mt(px(4.)).child(toolbar))
    }
}

/// `FabMenu::on_toggle` handler.
fn click_toggle(model: &Entity<Model>) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let model = model.clone();
    move |open, _, cx| {
        model.update(cx, |m, cx| {
            m.fab_open = open;
            event(if open { "fab-open" } else { "fab-close" });
            cx.notify();
        })
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The zero of every clip, and of the shared animation clock.
        mark_start();
        gpui_m3::motion::clock(cx);
        let s = *Theme::of(cx);
        let (queue_open, now_open, playlist_open, sleep_open, toast, draft) = {
            let m = self.model.read(cx);
            (
                m.queue_open,
                m.now_open,
                m.playlist_open,
                m.sleep_open,
                m.toast.clone(),
                m.sleep_draft,
            )
        };
        let model = self.model.clone();

        let width = animate_f32(
            "queue-width",
            gpui_m3::Spring::SPATIAL,
            if queue_open { queue::WIDTH + 8. } else { 0. },
            window,
            cx,
        )
        .max(0.);
        let page = div().relative().flex_1().min_w_0().h_full().child(
            div().absolute().top_0().left_0().size_full().child(
                AnyView::from(self.page.clone()).cached(StyleRefinement::default().size_full()),
            ),
        );
        let sheet = div()
            .flex_none()
            .w(px(width))
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .w(px(queue::WIDTH + 8.))
                    .h_full()
                    .pr(px(8.))
                    .pb(px(8.))
                    .child(
                        AnyView::from(self.queue.clone())
                            .cached(StyleRefinement::default().size_full()),
                    ),
            );

        let key_of = |name: &'static str| key(&gpui::ElementId::Name(name.into()), "presence");
        let now_p = presence(key_of("now"), now_open, window, cx);
        let playlist_p = presence(key_of("playlist"), playlist_open, window, cx);
        let sleep_p = presence(key_of("sleep"), sleep_open, window, cx);

        let name_field = self.name_field.clone();
        let close_now = click(&model, |m, cx| {
            event("now-playing-close");
            m.now_open = false;
            cx.notify();
        });
        let close_playlist = click(&model, |m, cx| {
            m.playlist_open = false;
            cx.notify();
        });
        let create = {
            let (model, field) = (model.clone(), name_field.clone());
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                let name = field.read(cx).text().trim().to_string();
                if name.is_empty() {
                    field.update(cx, |f, cx| {
                        f.set_error(Some("Name your playlist first".into()), cx)
                    });
                    return;
                }
                field.update(cx, |f, cx| {
                    f.set_error(None, cx);
                    f.set_text("", cx);
                });
                model.update(cx, |m, cx| {
                    m.playlists.push(name.clone().into());
                    m.playlist_open = false;
                    m.say(format!("\u{201c}{name}\u{201d} created"), cx);
                });
            }
        };

        div()
            .relative()
            .size_full()
            .flex()
            .bg(s.surface)
            .text_color(s.on_surface)
            .font_family(gpui_m3::theme::FONT_FAMILY)
            .child(self.rail(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().min_h_0().flex().child(page).child(sheet))
                    .child(self.bottom_band(cx)),
            )
            .when_some(now_p, |d, p| {
                d.child(
                    gpui_m3::BottomSheet::modal("now-sheet")
                        .max_width(px(960.))
                        .padding(px(24.))
                        .exit(p)
                        .on_dismiss(close_now)
                        .child(self.now.clone()),
                )
            })
            .when_some(playlist_p, |d, p| {
                d.child(
                    Dialog::new("playlist-dialog")
                        .title("New playlist")
                        .icon(icon("playlist-add"))
                        .width(px(420.))
                        .exit(p)
                        .on_dismiss(close_playlist)
                        .child(name_field.clone())
                        .action(
                            Button::new("playlist-cancel", "Cancel")
                                .text()
                                .on_click(click(&model, |m, cx| {
                                    m.playlist_open = false;
                                    cx.notify();
                                })),
                        )
                        .action(
                            Button::new("playlist-create", "Create")
                                .filled()
                                .on_click(create),
                        ),
                )
            })
            .when_some(sleep_p, |d, p| {
                d.child(
                    TimePicker::modal("sleep-picker")
                        .title("Sleep timer")
                        .twenty_four_hour(true)
                        .minute_step(5)
                        .value(draft)
                        .exit(p)
                        .on_change(model::change(&model, |m, t, cx| {
                            m.sleep_draft = t;
                            cx.notify();
                        }))
                        .on_confirm(click(&model, |m, cx| {
                            m.sleep = Some(m.sleep_draft);
                            m.sleep_open = false;
                            let text = format!("Sleep timer set for {}", m.sleep_draft);
                            m.say(text, cx);
                        }))
                        .on_cancel(click(&model, |m, cx| {
                            m.sleep_open = false;
                            cx.notify();
                        })),
                )
            })
            .when_some(toast, |d, (text, generation)| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(BAND + 8.))
                        .flex()
                        .justify_center()
                        .child(Toast::new(("toast", generation), text)),
                )
            })
            .child(ThemeReveal::new())
    }
}

fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn main() {
    Application::new()
        .with_assets(ShowcaseAssets)
        .run(|cx: &mut App| {
            gpui_m3::init(cx);
            if std::env::var("M3_REDUCED").is_ok_and(|v| v == "1") {
                cx.set_global(MotionSettings {
                    time_scale: 1.,
                    reduced: true,
                });
            }
            cx.set_global(Covers(ALBUMS.iter().map(|a| draw_cover(a, 440)).collect()));

            let mut state = Model::new();
            tour::configure(&mut state);
            if let Some(n) = env_f32("M3_SEED") {
                state.album = n as usize % ALBUMS.len();
                state.current = TRACKS
                    .iter()
                    .position(|t| t.album == state.album)
                    .unwrap_or(0);
                state.queue = model::queue_after(state.current);
            }
            state.playing = !std::env::var("SHOWCASE_PAUSED").is_ok_and(|v| v == "1");
            match std::env::var("M3_MODE").as_deref() {
                Ok("dark") => state.dark = true,
                Ok("light") => state.dark = false,
                _ => {}
            }
            if let Some(n) = env_f32("M3_VARIANT") {
                state.variant = SchemeVariant::ALL[n as usize % SchemeVariant::ALL.len()];
            }
            if let Some(c) = env_f32("M3_CONTRAST") {
                state.contrast = c.clamp(-1., 1.);
            }
            match std::env::var("M3_OPEN").as_deref() {
                Ok("now") => state.now_open = true,
                Ok("queue") => state.queue_open = true,
                Ok("fab") => state.fab_open = true,
                Ok("playlist") => state.playlist_open = true,
                Ok("sleep") => state.sleep_open = true,
                Ok("library") => state.nav = 2,
                _ => {}
            }
            let scheme = state.scheme();
            let dark = state.dark;
            Theme::set_scheme(scheme, dark, cx);
            let open_search = std::env::var("M3_OPEN").is_ok_and(|v| v == "search");

            let (width, height) = std::env::var("M3_SIZE")
                .ok()
                .and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
                })
                .unwrap_or((1280., 720.));
            let window_size = size(px(width), px(height));
            let bounds = match std::env::var("M3_POS").ok().and_then(|v| {
                let (x, y) = v.split_once(',')?;
                Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
            }) {
                Some((x, y)) => Bounds::new(point(px(x), px(y)), window_size),
                None => Bounds::centered(None, window_size, cx),
            };
            let frameless = std::env::var("SHOWCASE_FRAMELESS").is_ok_and(|v| v == "1");
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: (!frameless).then(|| TitlebarOptions {
                        title: Some("gpui-m3 · Music".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let model = cx.new(|_| state);
                    let shell = cx.new(|cx| Shell::new(model, window, cx));
                    if open_search {
                        let page = shell.read(cx).page.clone();
                        page.update(cx, |p, cx| p.open_search(window, cx));
                    }
                    shell
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
