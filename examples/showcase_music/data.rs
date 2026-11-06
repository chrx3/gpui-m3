//! Fake catalog and procedurally drawn album covers: no files, no network.

use std::f32::consts::TAU;
use std::sync::Arc;

use gpui::{Global, RenderImage, Rgba, hsla};
use gpui_m3::{ShapeName, theme::hex};

pub struct Album {
    pub name: &'static str,
    pub artist: &'static str,
    /// Seed color of the whole app's theme while this album plays.
    pub seed: &'static str,
    pub shape: ShapeName,
    /// Which cover drawing it gets.
    pub art: u8,
    /// Chill, Focus, Workout.
    pub moods: [bool; 3],
}

pub const ALBUMS: [Album; 8] = [
    Album {
        name: "Midnight Lagoon",
        artist: "Nova Reyes",
        seed: "#3b5bdb",
        shape: ShapeName::Cookie9,
        art: 0,
        moods: [true, true, false],
    },
    Album {
        name: "Golden Hour",
        artist: "The Amber Coast",
        seed: "#f08c00",
        shape: ShapeName::Sunny,
        art: 1,
        moods: [true, false, true],
    },
    Album {
        name: "Velvet Static",
        artist: "Lumen",
        seed: "#0c8599",
        shape: ShapeName::Clover8,
        art: 2,
        moods: [false, true, true],
    },
    Album {
        name: "Moss & Moonlight",
        artist: "Fern Hollow",
        seed: "#2f9e44",
        shape: ShapeName::Cookie6,
        art: 3,
        moods: [true, true, false],
    },
    Album {
        name: "Paper Planes",
        artist: "Kite Season",
        seed: "#e03131",
        shape: ShapeName::SoftBurst,
        art: 4,
        moods: [false, false, true],
    },
    Album {
        name: "Deep Blue Radio",
        artist: "Marina Sol",
        seed: "#7048e8",
        shape: ShapeName::Gem,
        art: 5,
        moods: [true, false, false],
    },
    Album {
        name: "Neon Garden",
        artist: "Orchid Club",
        seed: "#d6336c",
        shape: ShapeName::Flower,
        art: 6,
        moods: [false, true, true],
    },
    Album {
        name: "Slow Orbit",
        artist: "Halden",
        seed: "#1c7ed6",
        shape: ShapeName::Pebble,
        art: 7,
        moods: [true, true, false],
    },
];

pub struct Track {
    pub title: &'static str,
    pub album: usize,
    pub secs: u32,
}

const fn track(title: &'static str, album: usize, secs: u32) -> Track {
    Track { title, album, secs }
}

pub const TRACKS: [Track; 18] = [
    track("Glass Tide", 0, 222),
    track("Night Swim", 0, 245),
    track("Amber Road", 1, 206),
    track("Sunbleached", 1, 178),
    track("Static Bloom", 2, 272),
    track("Moth to Signal", 2, 227),
    track("Fern Song", 3, 213),
    track("Under the Canopy", 3, 302),
    track("Paper Planes", 4, 171),
    track("Kitefall", 4, 200),
    track("Sonar", 5, 235),
    track("Harbor Lights", 5, 260),
    track("Petal Pulse", 6, 192),
    track("After Hours", 6, 224),
    track("Perihelion", 7, 288),
    track("Drift", 7, 216),
    track("Low Lanterns", 0, 198),
    track("Last Light", 1, 251),
];

pub const MOODS: [&str; 4] = ["All", "Chill", "Focus", "Workout"];

/// Whether an album belongs to a mood chip (0 is "All").
pub fn in_mood(album: usize, mood: usize) -> bool {
    mood == 0 || ALBUMS[album].moods[mood - 1]
}

pub fn artist_of(track: usize) -> &'static str {
    ALBUMS[TRACKS[track].album].artist
}

/// "m:ss".
pub fn clock_text(secs: f32) -> String {
    let secs = secs.max(0.) as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Albums and tracks whose title, album or artist contain `query` (any case).
pub fn search(query: &str) -> (Vec<usize>, Vec<usize>) {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let albums = (0..ALBUMS.len())
        .filter(|&a| {
            ALBUMS[a].name.to_lowercase().contains(&query)
                || ALBUMS[a].artist.to_lowercase().contains(&query)
        })
        .collect();
    let tracks = (0..TRACKS.len())
        .filter(|&t| {
            TRACKS[t].title.to_lowercase().contains(&query)
                || ALBUMS[TRACKS[t].album].name.to_lowercase().contains(&query)
                || artist_of(t).to_lowercase().contains(&query)
        })
        .collect();
    (albums, tracks)
}

/// The drawn covers, one per album.
pub struct Covers(pub Vec<Arc<RenderImage>>);

impl Global for Covers {}

type Rgb = [f32; 3];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn color(h: f32, s: f32, l: f32) -> Rgb {
    let c = Rgba::from(hsla(h.rem_euclid(1.), s, l, 1.));
    [c.r, c.g, c.b]
}

/// Coverage of a shape from its signed distance, anti-aliased over `aa`.
fn cover(distance: f32, aa: f32) -> f32 {
    (0.5 - distance / aa).clamp(0., 1.)
}

fn ring(distance: f32, radius: f32, width: f32, aa: f32) -> f32 {
    cover((distance - radius).abs() - width / 2., aa)
}

/// One cover, `n` px square, drawn from the album's seed color.
pub fn draw_cover(album: &Album, n: usize) -> Arc<RenderImage> {
    let seed = hex(album.seed);
    let h = seed.h;
    let aa = 1.6 / n as f32;
    let mut data = vec![0u8; n * n * 4];
    for py in 0..n {
        for px in 0..n {
            let (x, y) = ((px as f32 + 0.5) / n as f32, (py as f32 + 0.5) / n as f32);
            let c = cover_pixel(album.art, h, x, y, aa);
            let o = (py * n + px) * 4;
            data[o] = (c[2] * 255.).round() as u8;
            data[o + 1] = (c[1] * 255.).round() as u8;
            data[o + 2] = (c[0] * 255.).round() as u8;
            data[o + 3] = 255;
        }
    }
    let buffer = image::RgbaImage::from_raw(n as u32, n as u32, data).expect("cover size");
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}

fn cover_pixel(art: u8, h: f32, x: f32, y: f32, aa: f32) -> Rgb {
    let dist = |cx: f32, cy: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    match art {
        // Sun over a striped sea.
        0 => {
            let sky = mix(color(h + 0.05, 0.7, 0.8), color(h, 0.75, 0.5), y * 1.3);
            let sun = cover(dist(0.5, 0.42) - 0.22, aa);
            let mut c = mix(sky, color(h + 0.1, 0.95, 0.86), sun);
            if y > 0.62 {
                let band = ((y - 0.62) * 22.).floor() as i32;
                let sea = color(h - 0.02, 0.75, 0.3 - 0.015 * band as f32);
                c = if (band % 2 == 0) || y > 0.86 {
                    sea
                } else {
                    mix(sea, c, 0.2)
                };
            }
            c
        }
        // Concentric rings from a corner.
        1 => {
            let d = dist(0.28, 0.72);
            let mut c = mix(color(h, 0.8, 0.62), color(h + 0.05, 0.85, 0.42), y);
            for k in 1..8 {
                let r = k as f32 * 0.115;
                let tone = if k % 2 == 0 {
                    color(h + 0.08, 0.95, 0.82)
                } else {
                    color(h - 0.03, 0.9, 0.28)
                };
                c = mix(c, tone, ring(d, r, 0.05, aa));
            }
            mix(c, color(h + 0.1, 1., 0.9), cover(d - 0.07, aa))
        }
        // Wave bands.
        2 => {
            let mut c = mix(color(h + 0.04, 0.5, 0.9), color(h, 0.6, 0.78), y);
            for k in 0..6 {
                let line = 0.28 + 0.115 * k as f32 + 0.05 * (x * 9. + k as f32 * 1.3).sin();
                let tone = color(h + 0.02 * k as f32, 0.8, 0.62 - 0.07 * k as f32);
                c = mix(c, tone, cover(line - y, aa * 1.5).min(1.) * 1.0);
            }
            c
        }
        // Night with soft discs.
        3 => {
            let mut c = mix(color(h, 0.55, 0.12), color(h + 0.05, 0.6, 0.26), 1. - y);
            for (cx, cy, r, hh, l) in [
                (0.7, 0.3, 0.2, 0.12, 0.88),
                (0.25, 0.65, 0.3, 0.0, 0.34),
                (0.62, 0.78, 0.16, 0.04, 0.5),
                (0.3, 0.2, 0.07, 0.1, 0.8),
            ] {
                let a = cover(dist(cx, cy) - r, aa * 3.);
                c = mix(c, color(h + hh, 0.7, l), a * 0.9);
            }
            c
        }
        // Folded paper: bold diagonal facets.
        4 => {
            let t = (x + y) / 2.;
            let facet = (((x - y) * 4.).floor() as i32).rem_euclid(3);
            let base = [
                color(h, 0.85, 0.58),
                color(h + 0.04, 0.9, 0.7),
                color(h - 0.03, 0.8, 0.4),
            ][facet as usize];
            let c = mix(base, color(h + 0.1, 0.9, 0.9), t * 0.25);
            mix(c, color(h, 0.3, 0.97), cover(dist(0.72, 0.3) - 0.13, aa))
        }
        // Radio dial: a grid of dots and a big offset disc.
        5 => {
            let mut c = mix(color(h, 0.7, 0.9), color(h + 0.06, 0.7, 0.8), x);
            let (gx, gy) = ((x * 14.).fract() - 0.5, (y * 14.).fract() - 0.5);
            c = mix(
                c,
                color(h, 0.8, 0.55),
                cover((gx * gx + gy * gy).sqrt() / 14. - 0.012, aa),
            );
            let disc = cover(dist(0.4, 0.58) - 0.3, aa);
            c = mix(c, color(h, 0.85, 0.34), disc);
            c = mix(
                c,
                color(h + 0.08, 0.9, 0.82),
                cover(dist(0.4, 0.58) - 0.1, aa),
            );
            mix(
                c,
                color(h, 0.85, 0.34),
                ring(dist(0.4, 0.58), 0.2, 0.012, aa),
            )
        }
        // Neon: dark ground, glowing arch and stripes.
        6 => {
            let mut c = mix(color(h, 0.5, 0.1), color(h + 0.04, 0.6, 0.2), y);
            for k in 0..5 {
                c = mix(
                    c,
                    color(h + 0.03 * k as f32, 0.95, 0.62 - 0.05 * k as f32),
                    ring(dist(0.5, 0.95), 0.2 + k as f32 * 0.12, 0.05, aa),
                );
            }
            let glow = (1. - dist(0.5, 0.95) * 1.4).clamp(0., 1.);
            mix(c, color(h + 0.1, 1., 0.7), glow * glow * 0.35)
        }
        // Orbit: petals around a planet.
        _ => {
            let (dx, dy) = (x - 0.5, y - 0.5);
            let r = (dx * dx + dy * dy).sqrt();
            let a = dy.atan2(dx);
            let petal = 0.27 + 0.09 * (a * 6.).cos();
            let mut c = mix(color(h + 0.05, 0.6, 0.86), color(h, 0.7, 0.66), r * 1.6);
            c = mix(c, color(h, 0.8, 0.4), cover(r - petal, aa));
            c = mix(c, color(h + 0.1, 0.9, 0.82), cover(r - 0.14, aa));
            mix(
                c,
                color(h, 0.8, 0.3),
                ring(r, 0.4 + 0.03 * (a * 3. + TAU / 3.).sin(), 0.012, aa),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_text_pads_the_seconds() {
        assert_eq!(clock_text(0.), "0:00");
        assert_eq!(clock_text(65.9), "1:05");
        assert_eq!(clock_text(-3.), "0:00");
    }

    #[test]
    fn search_matches_titles_albums_and_artists() {
        let (albums, tracks) = search("mo");
        assert_eq!(albums, [3]);
        assert!(tracks.contains(&5), "Moth to Signal");
        assert!(tracks.contains(&6), "Fern Song is on Moss & Moonlight");
        assert_eq!(search("  ").0.len() + search("").1.len(), 0);
        assert!(search("nova").0.contains(&0));
    }

    #[test]
    fn every_album_has_tracks_and_a_mood() {
        for a in 0..ALBUMS.len() {
            assert!(TRACKS.iter().any(|t| t.album == a), "{}", ALBUMS[a].name);
            assert!(ALBUMS[a].moods.iter().any(|m| *m));
            assert!(in_mood(a, 0));
        }
    }

    #[test]
    fn covers_have_the_requested_size() {
        for album in &ALBUMS {
            let image = draw_cover(album, 8);
            let size = image.size(0);
            assert_eq!((size.width.0, size.height.0), (8, 8));
        }
    }
}
