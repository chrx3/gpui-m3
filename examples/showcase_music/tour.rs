//! The scripted tour (`SHOWCASE_DEMO=1`) and the single-take clips (`SHOWCASE_CLIP=<name>`).
//!
//! Every step runs at a fixed time after the first frame drawn and calls the same functions the
//! mouse handlers call. Nothing is random, so two runs are the same.

use std::time::Duration;

use gpui::{AsyncWindowContext, Context, WeakEntity, Window};
use gpui_m3::{ReorderEvent, SchemeVariant};

use super::model::{Model, event, since_start};
use super::{Shell, set_dark};

/// What runs: the full tour, or one clip.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plan {
    Demo,
    Repaint,
    Carousel,
    Sheet,
    Toggles,
    Search,
    Queue,
    Fab,
    Palettes,
    Reveal,
}

fn plan() -> Option<Plan> {
    if std::env::var("SHOWCASE_DEMO").is_ok_and(|v| v == "1") {
        return Some(Plan::Demo);
    }
    Some(match std::env::var("SHOWCASE_CLIP").ok()?.as_str() {
        "repaint" => Plan::Repaint,
        "carousel" => Plan::Carousel,
        "sheet" => Plan::Sheet,
        "toggles" => Plan::Toggles,
        "search" => Plan::Search,
        "queue" => Plan::Queue,
        "fab" => Plan::Fab,
        "palettes" => Plan::Palettes,
        "reveal" => Plan::Reveal,
        other => {
            eprintln!("SHOWCASE_CLIP: unknown clip {other:?}");
            return None;
        }
    })
}

/// The state each script starts from.
pub fn configure(model: &mut Model) {
    match plan() {
        Some(Plan::Repaint) => {
            model.variant = SchemeVariant::Monochrome;
            model.dark = false;
        }
        Some(Plan::Toggles) => {
            model.now_open = true;
            model.normalize = [false, false];
        }
        Some(Plan::Palettes) => {
            model.album = 1;
            model.current = 2;
            model.queue = super::model::queue_after(2);
            model.dark = true;
        }
        Some(Plan::Reveal) => model.dark = true,
        _ => {}
    }
}

/// Starts the script, if one was asked for.
pub fn start(window: &mut Window, cx: &mut Context<Shell>) {
    let Some(plan) = plan() else {
        return;
    };
    cx.spawn_in(window, async move |this, cx| {
        // The zero of the timeline is the first frame drawn.
        while since_start().is_zero() {
            cx.background_executor()
                .timer(Duration::from_millis(5))
                .await;
        }
        let mut script = Script { this, cx };
        let completed = match plan {
            Plan::Demo => demo(&mut script).await,
            Plan::Repaint => repaint(&mut script).await,
            Plan::Carousel => carousel(&mut script).await,
            Plan::Sheet => sheet(&mut script).await,
            Plan::Toggles => toggles(&mut script).await,
            Plan::Search => search(&mut script).await,
            Plan::Queue => queue(&mut script).await,
            Plan::Fab => fab(&mut script).await,
            Plan::Palettes => palettes(&mut script).await,
            Plan::Reveal => reveal(&mut script).await,
        };
        if completed.is_none() {
            return;
        }
        if plan == Plan::Demo {
            println!("SHOWCASE_DEMO: done");
        } else {
            println!("SHOWCASE_CLIP: done");
            script.at_after(1000).await;
        }
        if std::env::var("SHOWCASE_EXIT").is_ok_and(|v| v == "1") {
            script.cx.update(|_, cx| cx.quit()).ok();
        }
    })
    .detach();
}

struct Script<'a> {
    this: WeakEntity<Shell>,
    cx: &'a mut AsyncWindowContext,
}

impl Script<'_> {
    /// Waits until `ms` after the first frame.
    async fn at(&mut self, ms: u64) {
        let now = since_start().as_millis() as u64;
        if now < ms {
            self.cx
                .background_executor()
                .timer(Duration::from_millis(ms - now))
                .await;
        }
    }

    async fn at_after(&mut self, ms: u64) {
        self.cx
            .background_executor()
            .timer(Duration::from_millis(ms))
            .await;
    }

    /// Runs `f` on the shell. `None` when the window is gone.
    fn run<R>(
        &mut self,
        f: impl FnOnce(&mut Shell, &mut Window, &mut Context<Shell>) -> R,
    ) -> Option<R> {
        self.this.update_in(self.cx, f).ok()
    }

    /// Prints the marker and runs `f`.
    fn step(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Shell, &mut Window, &mut Context<Shell>),
    ) -> Option<()> {
        event(name);
        self.run(f)
    }

    fn model(&mut self, name: &str, f: impl FnOnce(&mut Model, &mut Context<Model>)) -> Option<()> {
        self.step(name, |shell, _, cx| shell.model.update(cx, f))
    }
}

type Done = Option<()>;

/// Eases the page to `to` px.
fn scroll(script: &mut Script, name: &str, to: f32, millis: u64) -> Done {
    script.step(name, move |shell, _, cx| {
        shell
            .page
            .update(cx, |p, cx| p.animate_scroll(to, millis, cx))
    })
}

fn carousel_next(script: &mut Script, name: &str, which: usize) -> Done {
    script.step(name, move |shell, _, cx| {
        shell.page.update(cx, |p, cx| p.carousel_next(which, cx))
    })
}

fn open_search(script: &mut Script) -> Done {
    script.step("search-open", |shell, window, cx| {
        shell.page.update(cx, |p, cx| p.open_search(window, cx))
    })
}

fn type_search(script: &mut Script, text: &'static str) -> Done {
    script.step(&format!("search-type-{text}"), move |shell, _, cx| {
        let search = shell.page.read(cx).search.clone();
        search.update(cx, |s, cx| s.set_text(text, cx));
    })
}

fn close_search(script: &mut Script) -> Done {
    script.step("search-close", |shell, _, cx| {
        let search = shell.page.read(cx).search.clone();
        search.update(cx, |s, cx| s.close(cx));
    })
}

/// Empties the search field once its view has closed.
fn clear_search(script: &mut Script) -> Done {
    script.step("search-clear", |shell, _, cx| {
        let search = shell.page.read(cx).search.clone();
        search.update(cx, |s, cx| s.set_text("", cx));
    })
}

fn reveal_to(script: &mut Script, dark: bool) -> Done {
    script.run(move |shell, window, cx| {
        let origin = shell.toggle_origin.get();
        let model = shell.model.clone();
        set_dark(&model, dark, origin, window, cx);
    })
}

fn open_now(script: &mut Script) -> Done {
    script.model("now-playing-press", |m, cx| {
        m.now_open = true;
        cx.notify();
    })
}

/// A scripted drag of a queue row: lifted, a line where it will land, then dropped.
async fn drag_queue_row(
    script: &mut Script<'_>,
    from: usize,
    over: usize,
    after: bool,
    start_ms: u64,
) -> Done {
    script.at(start_ms).await;
    script.model("queue-drag-start", move |m, cx| {
        m.queue_held = Some((from, None));
        cx.notify();
    })?;
    script.at(start_ms + 350).await;
    script.model("queue-drag-over", move |m, cx| {
        m.queue_held = Some((from, Some((over, after))));
        cx.notify();
    })?;
    script.at(start_ms + 900).await;
    // Where the row ends up, as the reorder list works it out.
    let over_in_rest = if over > from { over - 1 } else { over };
    let to = over_in_rest + usize::from(after);
    script.model("queue-drop", move |m, cx| {
        m.queue_held = None;
        m.move_queue(ReorderEvent { from, to }, false, cx);
    })
}

fn variant(script: &mut Script, v: SchemeVariant) -> Done {
    script.run(move |shell, _, cx| shell.model.update(cx, |m, cx| m.set_variant(v, cx)))
}

async fn repaint(script: &mut Script<'_>) -> Done {
    script.at(1000).await;
    event("theme-blend-start");
    // Monochrome ignores the seed, so the first repaint also leaves it for Tonal spot.
    script.run(|shell, _, cx| {
        shell.model.update(cx, |m, cx| {
            m.variant = SchemeVariant::TonalSpot;
            m.select_album(1, cx);
        })
    })?;
    script.at(3200).await;
    event("theme-blend-start");
    script.run(|shell, _, cx| shell.model.update(cx, |m, cx| m.select_album(2, cx)))?;
    script.at(5500).await;
    event("clip-end");
    Some(())
}

async fn carousel(script: &mut Script<'_>) -> Done {
    for (i, ms) in [1000, 2200, 3400].into_iter().enumerate() {
        script.at(ms).await;
        carousel_next(script, &format!("carousel-next-{}", i + 1), 0)?;
    }
    script.at(5000).await;
    event("clip-end");
    Some(())
}

/// Equalizer moves: band, target gain and the time of the first one.
const EQ_MOVES: [(usize, f32); 5] = [(0, 9.), (1, -6.), (2, 6.), (3, -5.), (4, 8.)];

fn eq_moves(script: &mut Script) -> Done {
    script.run(|shell, _, cx| {
        shell.model.update(cx, |m, cx| {
            for (i, (band, target)) in EQ_MOVES.into_iter().enumerate() {
                m.animate_eq(band, target, i as u64 * 250, 450, cx);
            }
        })
    })
}

async fn sheet(script: &mut Script<'_>) -> Done {
    script.at(800).await;
    open_now(script)?;
    script.at(2200).await;
    event("eq-start");
    eq_moves(script)?;
    script.at(6000).await;
    event("clip-end");
    Some(())
}

async fn toggles(script: &mut Script<'_>) -> Done {
    script.at(700).await;
    script.model("lossless-on", |m, cx| {
        m.lossless = true;
        cx.notify();
    })?;
    script.at(1400).await;
    script.model("spatial-on", |m, cx| {
        m.spatial = true;
        cx.notify();
    })?;
    script.at(2100).await;
    script.model("shuffle-on", |m, cx| {
        m.shuffle = true;
        cx.notify();
    })?;
    script.at(2800).await;
    script.model("repeat-on", |m, cx| {
        m.repeat = true;
        cx.notify();
    })?;
    script.at(3500).await;
    script.model("normalize-indeterminate", |m, cx| {
        m.normalize = [true, false];
        cx.notify();
    })?;
    script.at(4200).await;
    script.model("normalize-checked", |m, cx| {
        m.normalize = [true, true];
        cx.notify();
    })?;
    script.at(5500).await;
    event("clip-end");
    Some(())
}

async fn search(script: &mut Script<'_>) -> Done {
    script.at(800).await;
    open_search(script)?;
    script.at(1800).await;
    type_search(script, "m")?;
    script.at(2050).await;
    type_search(script, "mo")?;
    script.at(4500).await;
    close_search(script)?;
    script.at(5000).await;
    clear_search(script)?;
    script.at(5500).await;
    event("clip-end");
    Some(())
}

async fn queue(script: &mut Script<'_>) -> Done {
    script.at(800).await;
    script.model("queue-open", |m, cx| {
        m.queue_open = true;
        cx.notify();
    })?;
    drag_queue_row(script, 3, 0, false, 2000).await?;
    drag_queue_row(script, 4, 6, true, 3300).await?;
    script.at(5000).await;
    script.model("queue-close", |m, cx| {
        m.queue_open = false;
        cx.notify();
    })?;
    script.at(6000).await;
    event("clip-end");
    Some(())
}

async fn fab(script: &mut Script<'_>) -> Done {
    script.at(800).await;
    script.model("fab-open", |m, cx| {
        m.fab_open = true;
        cx.notify();
    })?;
    script.at(2800).await;
    script.model("fab-press-import", |m, cx| {
        m.fab_open = false;
        m.say("Imported 12 songs from Downloads", cx);
    })?;
    script.at(4500).await;
    event("clip-end");
    Some(())
}

async fn palettes(script: &mut Script<'_>) -> Done {
    // The palette buttons are at the end of the page.
    script.at(200).await;
    scroll(script, "scroll-to-appearance", 9999., 700)?;
    for (v, ms) in [
        (SchemeVariant::Vibrant, 1000),
        (SchemeVariant::Expressive, 2100),
        (SchemeVariant::Rainbow, 3200),
        (SchemeVariant::Monochrome, 4300),
    ] {
        script.at(ms).await;
        variant(script, v)?;
    }
    script.at(5400).await;
    script.model("contrast-high", |m, cx| m.set_contrast(1., cx))?;
    script.at(7000).await;
    event("clip-end");
    Some(())
}

async fn reveal(script: &mut Script<'_>) -> Done {
    script.at(1000).await;
    reveal_to(script, false)?;
    script.at(3500).await;
    reveal_to(script, true)?;
    script.at(6000).await;
    event("clip-end");
    Some(())
}

/// The whole tour, about a minute.
async fn demo(script: &mut Script<'_>) -> Done {
    // 1. The app opens and music plays.
    script.at(2500).await;
    // 2. The page scrolls (the large bar collapses) and the carousels move.
    scroll(script, "page-scroll-down", 330., 1500)?;
    script.at(3300).await;
    carousel_next(script, "carousel-made-1", 0)?;
    script.at(4500).await;
    carousel_next(script, "carousel-made-2", 0)?;
    script.at(5700).await;
    carousel_next(script, "carousel-made-3", 0)?;
    script.at(6600).await;
    scroll(script, "page-scroll-recent", 560., 1000)?;
    carousel_next(script, "carousel-recent-1", 1)?;
    script.at(8100).await;
    carousel_next(script, "carousel-recent-2", 1)?;
    script.at(9000).await;
    scroll(script, "page-scroll-top", 0., 1400)?;
    // 3. Search: "mo", one key at a time.
    script.at(11000).await;
    open_search(script)?;
    script.at(12000).await;
    type_search(script, "m")?;
    script.at(12300).await;
    type_search(script, "mo")?;
    script.at(14500).await;
    close_search(script)?;
    script.at(15300).await;
    clear_search(script)?;
    // 4. The theme follows the albums, then the palette variants.
    script.at(16000).await;
    script.run(|shell, _, cx| shell.model.update(cx, |m, cx| m.select_album(1, cx)))?;
    script.at(18000).await;
    script.run(|shell, _, cx| shell.model.update(cx, |m, cx| m.select_album(2, cx)))?;
    script.at(20000).await;
    script.run(|shell, _, cx| shell.model.update(cx, |m, cx| m.select_album(3, cx)))?;
    for (v, ms) in [
        (SchemeVariant::Vibrant, 22000),
        (SchemeVariant::Expressive, 23800),
        (SchemeVariant::Monochrome, 25600),
        (SchemeVariant::TonalSpot, 27400),
    ] {
        script.at(ms).await;
        variant(script, v)?;
    }
    // 5. The queue slides in and two tracks are reordered.
    script.at(29000).await;
    script.model("queue-open", |m, cx| {
        m.queue_open = true;
        cx.notify();
    })?;
    drag_queue_row(script, 3, 0, false, 30800).await?;
    drag_queue_row(script, 4, 6, true, 33000).await?;
    script.at(35200).await;
    script.model("queue-close", |m, cx| {
        m.queue_open = false;
        cx.notify();
    })?;
    // 6. Now playing: equalizer, a switch, shuffle.
    script.at(36600).await;
    open_now(script)?;
    script.at(38600).await;
    event("eq-start");
    eq_moves(script)?;
    script.at(41200).await;
    script.model("spatial-on", |m, cx| {
        m.spatial = true;
        cx.notify();
    })?;
    script.at(42200).await;
    script.model("shuffle-on", |m, cx| {
        m.shuffle = true;
        cx.notify();
    })?;
    // 7. The sheet closes and the FAB menu opens and closes.
    script.at(43800).await;
    script.model("now-playing-close", |m, cx| {
        m.now_open = false;
        cx.notify();
    })?;
    script.at(45600).await;
    script.model("fab-open", |m, cx| {
        m.fab_open = true;
        cx.notify();
    })?;
    script.at(48000).await;
    script.model("fab-close", |m, cx| {
        m.fab_open = false;
        cx.notify();
    })?;
    // 8. Dark, then light again, with the circular reveal from the theme button.
    script.at(49800).await;
    reveal_to(script, true)?;
    script.at(53400).await;
    reveal_to(script, false)?;
    // 9. Hold the final state.
    script.at(60000).await;
    event("tour-end");
    Some(())
}
