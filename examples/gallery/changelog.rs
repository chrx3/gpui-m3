//! "What's new" section: the release notes of every version, read from `changelog/changelog.json`.
//!
//! The same file feeds `CHANGELOG.md` and the GitHub release notes, so a version is written once.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Render, SharedString, Styled, StyledImage, WeakEntity, Window, div, img, px,
};
use gpui_m3::{Badge, Button, ButtonSize, Segment, SegmentedButtons, Theme, Tone};
use serde::Deserialize;

use super::{Demos, column, row};

const CHANGELOG: &str = include_str!("../../changelog/changelog.json");

/// Languages of the notes, in the order of the language switch.
const LANGUAGES: [&str; 2] = ["ES", "EN"];

#[derive(Deserialize)]
struct Log {
    versions: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    version: String,
    /// `YYYY-MM-DD`.
    date: String,
    title: Option<Text>,
    items: Vec<Entry>,
}

#[derive(Deserialize)]
struct Text {
    es: String,
    en: String,
}

#[derive(Deserialize)]
struct Entry {
    kind: Kind,
    es: String,
    en: String,
    /// Path of a screenshot, relative to the repository root.
    image: Option<String>,
    /// Gallery section that shows the component (index into `SECTIONS`).
    section: Option<usize>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Kind {
    New,
    Improved,
    Fixed,
}

impl Kind {
    fn label(self, es: bool) -> &'static str {
        match (self, es) {
            (Kind::New, true) => "Nuevo",
            (Kind::New, false) => "New",
            (Kind::Improved, true) => "Mejorado",
            (Kind::Improved, false) => "Improved",
            (Kind::Fixed, true) => "Corregido",
            (Kind::Fixed, false) => "Fixed",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Kind::New => Tone::Primary,
            Kind::Improved => Tone::Secondary,
            Kind::Fixed => Tone::Tertiary,
        }
    }
}

fn log() -> Log {
    serde_json::from_str(CHANGELOG).expect("changelog/changelog.json is not valid")
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Width over height of a PNG, from its header (no decoding): the picture must take its height
/// from the width the card leaves it.
fn png_ratio(path: &Path) -> Option<f32> {
    let mut header = [0u8; 24];
    File::open(path).ok()?.read_exact(&mut header).ok()?;
    if header[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return None;
    }
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (height > 0).then(|| width as f32 / height as f32)
}

/// `2026-10-10` as "10 oct 2026" or "Oct 10, 2026".
fn pretty_date(date: &str, es: bool) -> String {
    const ES: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = date.split('-').map(|p| p.parse::<usize>().ok());
    match (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    ) {
        (Some(year), Some(month @ 1..=12), Some(day)) => {
            if es {
                format!("{day} {} {year}", ES[month - 1])
            } else {
                format!("{} {day}, {year}", EN[month - 1])
            }
        }
        _ => date.to_string(),
    }
}

pub struct Changelog {
    log: Log,
    /// Index into [`LANGUAGES`]; Spanish first.
    language: usize,
    /// Aspect ratio of each picture that exists, by its path in the log.
    ratios: HashMap<String, f32>,
    /// Opens a gallery section from the "see in the gallery" buttons.
    demos: WeakEntity<Demos>,
}

impl Changelog {
    pub fn new(demos: WeakEntity<Demos>) -> Self {
        let log = log();
        let ratios = log
            .versions
            .iter()
            .flat_map(|release| &release.items)
            .filter_map(|entry| entry.image.as_ref())
            .filter_map(|image| Some((image.clone(), png_ratio(&repo_path(image))?)))
            .collect();
        Self {
            log,
            language: 0,
            ratios,
            demos,
        }
    }

    fn release(&self, index: usize, release: &Release, cx: &Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let es = self.language == 0;
        let latest = index == 0;

        let mut header = row().gap(px(10.)).child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight(700.))
                .child(format!("v{}", release.version)),
        );
        if latest {
            header = header.child(Badge::tonal(
                Tone::Primary,
                if es { "Última" } else { "Latest" },
            ));
        }
        header = header.child(
            div()
                .text_size(px(12.5))
                .text_color(s.on_surface_variant)
                .child(pretty_date(&release.date, es)),
        );

        let mut card = column()
            .gap(px(18.))
            .p(px(24.))
            .rounded(px(28.))
            .bg(if latest {
                s.surface_container_high
            } else {
                s.surface_container
            })
            .child(header);
        if let Some(title) = &release.title {
            card = card.child(
                div()
                    .text_size(px(15.))
                    .text_color(s.on_surface_variant)
                    .child(if es {
                        title.es.clone()
                    } else {
                        title.en.clone()
                    }),
            );
        }
        for (i, entry) in release.items.iter().enumerate() {
            card = card.child(self.entry(index * 1000 + i, entry));
        }
        card.into_any_element()
    }

    fn entry(&self, id: usize, entry: &Entry) -> AnyElement {
        let es = self.language == 0;
        let text = if es { &entry.es } else { &entry.en };

        let mut line = row().gap(px(12.)).items_start().child(
            div()
                .w(px(86.))
                .flex_none()
                .child(Badge::tonal(entry.kind.tone(), entry.kind.label(es))),
        );
        line = line.child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(14.))
                .child(SharedString::from(text.clone())),
        );
        if let Some(section) = entry.section {
            let demos = self.demos.clone();
            line = line.child(
                Button::new(
                    ("changelog-open", id),
                    if es {
                        "Ver en el gallery"
                    } else {
                        "See in the gallery"
                    },
                )
                .tonal()
                .size(ButtonSize::Small)
                .on_click(move |_, _, cx| {
                    demos
                        .update(cx, |demos, cx| {
                            demos.section = section;
                            cx.notify();
                        })
                        .ok();
                }),
            );
        }

        let mut item = column().gap(px(12.)).child(line);
        if let Some((image, ratio)) = entry
            .image
            .as_deref()
            .and_then(|image| Some((repo_path(image), *self.ratios.get(image)?)))
        {
            // The box takes its height from the width the card leaves it; the picture fills it.
            let mut frame = div()
                .w_full()
                .rounded(px(16.))
                .overflow_hidden()
                .child(img(image).size_full().object_fit(ObjectFit::Contain));
            frame.style().aspect_ratio = Some(ratio);
            item = item.child(div().pl(px(98.)).max_w(px(738.)).min_w_0().child(frame));
        }
        item.into_any_element()
    }
}

impl Render for Changelog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let es = self.language == 0;
        let view = cx.entity().downgrade();

        let toolbar = row()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child(if es {
                        "Lo que trae cada versión de gpui-m3."
                    } else {
                        "What each gpui-m3 version brings."
                    }),
            )
            .child(
                SegmentedButtons::new(
                    "changelog-language",
                    LANGUAGES.map(Segment::label),
                    self.language,
                )
                .sliding(true)
                .on_change(move |language, _, cx| {
                    view.update(cx, |this, cx| {
                        this.language = language;
                        cx.notify();
                    })
                    .ok();
                }),
            );

        let mut releases = column().gap(px(20.));
        for (i, release) in self.log.versions.iter().enumerate() {
            releases = releases.child(self.release(i, release, cx));
        }

        column()
            .id("changelog")
            .max_w(px(820.))
            .pt(px(4.))
            .gap(px(20.))
            .child(toolbar)
            .child(releases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_is_valid_and_newest_first() {
        let log = log();
        assert!(!log.versions.is_empty());
        for pair in log.versions.windows(2) {
            assert!(
                pair[0].date >= pair[1].date,
                "{} is listed before {}",
                pair[0].version,
                pair[1].version
            );
        }
        for release in &log.versions {
            assert!(
                !release.items.is_empty(),
                "{} has no items",
                release.version
            );
            for entry in &release.items {
                assert!(!entry.es.is_empty() && !entry.en.is_empty());
            }
        }
    }

    #[test]
    fn the_current_version_has_its_notes() {
        let log = log();
        assert!(
            log.versions
                .iter()
                .any(|r| r.version == env!("CARGO_PKG_VERSION")),
            "changelog/changelog.json has no entry for {}",
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn images_and_sections_exist() {
        for release in &log().versions {
            for entry in &release.items {
                if let Some(image) = &entry.image {
                    assert!(repo_path(image).exists(), "missing image {image}");
                }
                if let Some(section) = entry.section {
                    assert!(
                        section < super::super::SECTIONS.len(),
                        "no section {section}"
                    );
                }
            }
        }
    }

    #[test]
    fn dates_are_readable() {
        assert_eq!(pretty_date("2026-10-10", true), "10 oct 2026");
        assert_eq!(pretty_date("2026-10-10", false), "Oct 10, 2026");
        assert_eq!(pretty_date("pronto", true), "pronto");
    }
}
