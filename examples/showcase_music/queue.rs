//! The queue: a standard side sheet with a list you can reorder by dragging.

use gpui::{App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};
use gpui_m3::{
    Button, Icon, ListItem, ReorderEvent, ReorderList, Shape, SideSheet, Theme,
    theme::type_scale::{BODY_MEDIUM, TypeScaleExt},
};

use super::data::{ALBUMS, Covers, TRACKS, artist_of, clock_text};
use super::icons::icon;
use super::model::{Model, click};

/// Width of the sheet.
pub const WIDTH: f32 = 340.;

pub struct QueuePanel {
    model: Entity<Model>,
}

impl QueuePanel {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model }
    }
}

impl Render for QueuePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (queue, moved, held) = {
            let m = self.model.read(cx);
            (m.queue.clone(), m.queue_move, m.queue_held)
        };
        let model = self.model.clone();
        let total: u32 = queue.iter().map(|&t| TRACKS[t].secs).sum();

        let mut list = ReorderList::new("queue-list").gap(px(2.)).header("Up next");
        for &track in &queue {
            let play = self.model.clone();
            list = list.row(
                format!("q{track}"),
                0,
                ListItem::new(("queue-row", track), TRACKS[track].title)
                    .subtitle(format!(
                        "{} · {}",
                        artist_of(track),
                        clock_text(TRACKS[track].secs as f32)
                    ))
                    .leading(
                        Shape::new(ALBUMS[TRACKS[track].album].shape)
                            .image(cx.global::<Covers>().0[TRACKS[track].album].clone())
                            .size(px(40.)),
                    )
                    .trailing(
                        Icon::new(icon("grip"))
                            .size(px(18.))
                            .color(s.on_surface_variant),
                    )
                    .on_click(move |_, _, cx| play.update(cx, |m, cx| m.play_track(track, cx))),
            );
        }
        if let Some((event, generation)) = moved {
            list = list.moved(event, generation);
        }
        if let Some((from, target)) = held {
            list = list.held(from, target);
        }
        let list = list.on_reorder({
            let model = self.model.clone();
            move |event: ReorderEvent, _: &mut Window, cx: &mut App| {
                model.update(cx, |m, cx| m.move_queue(event, true, cx))
            }
        });

        SideSheet::standard("queue-sheet")
            .width(px(WIDTH))
            .title("Queue")
            .on_close(click(&model, |m, cx| {
                m.queue_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .pb(px(8.))
                    .px(px(12.))
                    .type_style(BODY_MEDIUM)
                    .text_color(s.on_surface_variant)
                    .child(format!(
                        "{} songs · {} min up next",
                        queue.len(),
                        total / 60
                    )),
            )
            .child(list)
            .action(
                Button::new("queue-clear", "Clear")
                    .text()
                    .on_click(click(&model, |m, cx| {
                        m.queue.clear();
                        cx.notify();
                    })),
            )
            .action(
                Button::new("queue-save", "Save as playlist")
                    .filled()
                    .on_click(click(&model, |m, cx| {
                        m.playlist_open = true;
                        cx.notify();
                    })),
            )
    }
}
