//! What the views share: the playback state and the look of the app.

use std::sync::OnceLock;
use std::time::Instant;

use gpui::{App, ClickEvent, Context, Entity, SharedString, Window};
use gpui_m3::{
    ReorderEvent, Scheme, SchemeVariant, Time, animate_scheme, apply_reorder, theme::hex,
};

use super::data::{ALBUMS, TRACKS};

/// Prints a line for the recording: milliseconds since the first frame drawn, and a name.
pub fn event(name: &str) {
    println!("SHOWCASE_EVENT t={}ms {name}", since_start().as_millis());
}

static START: OnceLock<Instant> = OnceLock::new();

/// Marks the first frame drawn (the zero of every clip's timeline). Later calls do nothing.
pub fn mark_start() {
    START.get_or_init(Instant::now);
}

pub fn since_start() -> std::time::Duration {
    START.get().map_or_else(Default::default, Instant::elapsed)
}

/// Where the track is, apart from [`Model`] so that its ticks redraw only the progress bars.
pub struct Clock {
    pub position: f32,
}

/// Seconds into the first track when the app opens, so every run starts from the same place.
pub const START_POSITION: f32 = 41.;

/// The next track and queue after "next": the queue moves up and the old track goes to its end.
pub fn advance(current: usize, queue: &[usize]) -> (usize, Vec<usize>) {
    let Some((&first, rest)) = queue.split_first() else {
        return (current, Vec::new());
    };
    let mut queue = rest.to_vec();
    queue.push(current);
    (first, queue)
}

/// The reverse of [`advance`].
pub fn rewind(current: usize, queue: &[usize]) -> (usize, Vec<usize>) {
    let Some((&last, rest)) = queue.split_last() else {
        return (current, Vec::new());
    };
    let mut queue = vec![current];
    queue.extend_from_slice(rest);
    (last, queue)
}

/// Upcoming tracks when `current` plays: the other tracks, the album's own first.
pub fn queue_after(current: usize) -> Vec<usize> {
    let album = TRACKS[current].album;
    let mut rest: Vec<usize> = (0..TRACKS.len()).filter(|&t| t != current).collect();
    rest.sort_by_key(|&t| (TRACKS[t].album != album, t));
    rest.truncate(7);
    rest
}

pub struct Model {
    /// The album whose seed colors the whole app.
    pub album: usize,
    pub current: usize,
    pub playing: bool,
    pub queue: Vec<usize>,
    /// The last move made by hand or by script, for the queue's slide animation.
    pub queue_move: Option<(ReorderEvent, u64)>,
    /// A row shown as dragged (scripted drag).
    pub queue_held: Option<(usize, Option<(usize, bool)>)>,
    pub liked: [bool; TRACKS.len()],
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub dark: bool,
    pub variant: SchemeVariant,
    pub contrast: f32,
    /// Rail destination: 0 Home, 1 Search, 2 Library.
    pub nav: usize,
    pub mood: usize,
    pub queue_open: bool,
    pub now_open: bool,
    pub fab_open: bool,
    pub playlist_open: bool,
    pub sleep_open: bool,
    pub eq: [f32; 5],
    pub preset: usize,
    pub bass: (f32, f32),
    pub lossless: bool,
    pub spatial: bool,
    pub normalize: [bool; 2],
    pub playlists: Vec<SharedString>,
    pub sleep: Option<Time>,
    pub sleep_draft: Time,
    pub toast: Option<(SharedString, u64)>,
}

impl Model {
    pub fn new() -> Self {
        let mut liked = [false; TRACKS.len()];
        liked[0] = true;
        liked[6] = true;
        Self {
            album: 0,
            current: 0,
            playing: true,
            queue: queue_after(0),
            queue_move: None,
            queue_held: None,
            liked,
            volume: 72.,
            shuffle: false,
            repeat: false,
            dark: false,
            variant: SchemeVariant::TonalSpot,
            contrast: 0.,
            nav: 0,
            mood: 0,
            queue_open: false,
            now_open: false,
            fab_open: false,
            playlist_open: false,
            sleep_open: false,
            eq: [3., 1., -1., 2., 4.],
            preset: 0,
            bass: (20., 180.),
            lossless: false,
            spatial: false,
            normalize: [true, false],
            playlists: vec!["Late night drive".into(), "Sunday reset".into()],
            sleep: None,
            sleep_draft: Time::new(0, 30).expect("valid time"),
            toast: None,
        }
    }

    /// The scheme of the current album, mode, variant and contrast.
    pub fn scheme(&self) -> Scheme {
        Scheme::from_seed_variant(
            hex(ALBUMS[self.album].seed),
            self.dark,
            self.variant,
            self.contrast,
        )
    }

    /// Blends the app into the current look.
    pub fn apply_theme(&self, cx: &mut Context<Self>) {
        animate_scheme(self.scheme(), self.dark, cx);
    }

    /// Plays `track` of its album: the theme follows the album.
    pub fn play_track(&mut self, track: usize, cx: &mut Context<Self>) {
        let album = TRACKS[track].album;
        self.current = track;
        self.queue = queue_after(track);
        self.playing = true;
        if album != self.album {
            self.album = album;
            self.apply_theme(cx);
        }
        cx.notify();
    }

    /// Plays the first track of an album (pressing a cover).
    pub fn select_album(&mut self, album: usize, cx: &mut Context<Self>) {
        let track = TRACKS
            .iter()
            .position(|t| t.album == album)
            .unwrap_or(self.current);
        event(&format!("select-album-{}", album + 1));
        self.play_track(track, cx);
    }

    pub fn next(&mut self, cx: &mut Context<Self>) {
        let (current, queue) = advance(self.current, &self.queue);
        self.set_current(current, queue, cx);
    }

    pub fn previous(&mut self, cx: &mut Context<Self>) {
        let (current, queue) = rewind(self.current, &self.queue);
        self.set_current(current, queue, cx);
    }

    fn set_current(&mut self, current: usize, queue: Vec<usize>, cx: &mut Context<Self>) {
        self.current = current;
        self.queue = queue;
        let album = TRACKS[current].album;
        if album != self.album {
            self.album = album;
            self.apply_theme(cx);
        }
        cx.notify();
    }

    pub fn set_variant(&mut self, variant: SchemeVariant, cx: &mut Context<Self>) {
        self.variant = variant;
        event(&format!(
            "variant-{}",
            variant.label().to_lowercase().replace(' ', "-")
        ));
        self.apply_theme(cx);
        cx.notify();
    }

    pub fn set_contrast(&mut self, contrast: f32, cx: &mut Context<Self>) {
        self.contrast = contrast;
        self.apply_theme(cx);
        cx.notify();
    }

    /// Moves a queue row (by hand or by script) and remembers it for the slide.
    pub fn move_queue(&mut self, event: ReorderEvent, by_hand: bool, cx: &mut Context<Self>) {
        apply_reorder(&mut self.queue, event);
        if !by_hand {
            let generation = self.queue_move.map_or(1, |(_, g)| g + 1);
            self.queue_move = Some((event, generation));
        }
        cx.notify();
    }

    pub fn set_eq(&mut self, band: usize, value: f32, cx: &mut Context<Self>) {
        self.eq[band] = value;
        self.preset = 4;
        cx.notify();
    }

    /// Moves one equalizer band to `target` over `millis`, after `delay` ms (a scripted slide).
    pub fn animate_eq(
        &mut self,
        band: usize,
        target: f32,
        delay: u64,
        millis: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(delay))
                .await;
            event(&format!("eq-band-{}", band + 1));
            let Ok(from) = this.read_with(cx, |m, _| m.eq[band]) else {
                return;
            };
            let start = Instant::now();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let t = (start.elapsed().as_secs_f32() * 1000. / millis.max(1) as f32).min(1.);
                let eased = t * t * (3. - 2. * t);
                let alive = this
                    .update(cx, |m, cx| {
                        m.set_eq(band, from + (target - from) * eased, cx)
                    })
                    .is_ok();
                if !alive || t >= 1. {
                    break;
                }
            }
        })
        .detach();
    }

    /// Shows a toast for a couple of seconds.
    pub fn say(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let generation = self.toast.as_ref().map_or(1, |(_, g)| g + 1);
        self.toast = Some((text.into(), generation));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(2600))
                .await;
            this.update(cx, |m, cx| {
                if m.toast.as_ref().is_some_and(|(_, g)| *g == generation) {
                    m.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}

/// A click handler that updates the model.
pub fn click(
    model: &Entity<Model>,
    f: impl Fn(&mut Model, &mut Context<Model>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let model = model.clone();
    move |_, _, cx| model.update(cx, |m, cx| f(m, cx))
}

/// A value-change handler that updates the model.
pub fn change<T: 'static>(
    model: &Entity<Model>,
    f: impl Fn(&mut Model, T, &mut Context<Model>) + 'static,
) -> impl Fn(T, &mut Window, &mut App) + 'static {
    let model = model.clone();
    move |value, _, cx| model.update(cx, |m, cx| f(m, value, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_rotates_the_queue_and_previous_undoes_it() {
        let (current, queue) = advance(1, &[2, 3, 4]);
        assert_eq!((current, queue.as_slice()), (2, &[3, 4, 1][..]));
        let (current, queue) = rewind(current, &queue);
        assert_eq!((current, queue.as_slice()), (1, &[2, 3, 4][..]));
    }

    #[test]
    fn an_empty_queue_keeps_the_track() {
        assert_eq!(advance(5, &[]), (5, Vec::new()));
        assert_eq!(rewind(5, &[]), (5, Vec::new()));
    }

    #[test]
    fn the_queue_starts_with_the_albums_own_tracks() {
        let queue = queue_after(0);
        assert_eq!(queue.len(), 7);
        assert!(!queue.contains(&0));
        assert_eq!(TRACKS[queue[0]].album, 0);
        assert_eq!(TRACKS[queue[1]].album, 0);
    }
}
