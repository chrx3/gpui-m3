//! Icons embedded in the binary (24 px strokes, the same set as the reference design).
//! They are requested as `m3/icons/<name>.svg`, so they never clash with the host app's
//! own `icons/`; see [`crate::Icon`].

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// The library's asset source. To combine it with your app's, delegate
/// `m3/…` paths to it or use [`Assets::load_icon`].
pub struct Assets;

impl Assets {
    pub fn load_icon(path: &str) -> Option<&'static [u8]> {
        let name = path.strip_prefix(ICON_PREFIX)?.strip_suffix(".svg")?;
        ICONS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, bytes)| *bytes)
    }

    /// Names of all available icons.
    pub fn icon_names() -> impl Iterator<Item = &'static str> {
        ICONS.iter().map(|(n, _)| *n)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::load_icon(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path.starts_with("m3") {
            ICONS
                .iter()
                .map(|(n, _)| format!("{ICON_PREFIX}{n}.svg").into())
                .collect()
        } else {
            Vec::new()
        })
    }
}

/// Asset path prefix of the library icons.
pub const ICON_PREFIX: &str = "m3/icons/";

const ICONS: &[(&str, &[u8])] = &[
    (
        "arrow-left",
        include_bytes!("../assets/icons/arrow-left.svg"),
    ),
    ("at", include_bytes!("../assets/icons/at.svg")),
    ("blocks", include_bytes!("../assets/icons/blocks.svg")),
    ("bolt", include_bytes!("../assets/icons/bolt.svg")),
    ("bookmark", include_bytes!("../assets/icons/bookmark.svg")),
    ("box", include_bytes!("../assets/icons/box.svg")),
    ("brain", include_bytes!("../assets/icons/brain.svg")),
    ("branch", include_bytes!("../assets/icons/branch.svg")),
    ("chat", include_bytes!("../assets/icons/chat.svg")),
    ("check", include_bytes!("../assets/icons/check.svg")),
    (
        "chevron-down",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "chevron-right",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    ("clip", include_bytes!("../assets/icons/clip.svg")),
    ("command", include_bytes!("../assets/icons/command.svg")),
    ("copy", include_bytes!("../assets/icons/copy.svg")),
    ("cpu", include_bytes!("../assets/icons/cpu.svg")),
    ("diff", include_bytes!("../assets/icons/diff.svg")),
    ("export", include_bytes!("../assets/icons/export.svg")),
    ("file", include_bytes!("../assets/icons/file.svg")),
    ("files", include_bytes!("../assets/icons/files.svg")),
    (
        "folder-open",
        include_bytes!("../assets/icons/folder-open.svg"),
    ),
    (
        "folder-plus",
        include_bytes!("../assets/icons/folder-plus.svg"),
    ),
    ("folder", include_bytes!("../assets/icons/folder.svg")),
    ("gauge", include_bytes!("../assets/icons/gauge.svg")),
    ("gear", include_bytes!("../assets/icons/gear.svg")),
    ("history", include_bytes!("../assets/icons/history.svg")),
    ("hook", include_bytes!("../assets/icons/hook.svg")),
    ("import", include_bytes!("../assets/icons/import.svg")),
    ("layers", include_bytes!("../assets/icons/layers.svg")),
    ("monitor", include_bytes!("../assets/icons/monitor.svg")),
    ("moon", include_bytes!("../assets/icons/moon.svg")),
    ("more", include_bytes!("../assets/icons/more.svg")),
    ("palette", include_bytes!("../assets/icons/palette.svg")),
    (
        "panel-bottom",
        include_bytes!("../assets/icons/panel-bottom.svg"),
    ),
    (
        "panel-left",
        include_bytes!("../assets/icons/panel-left.svg"),
    ),
    (
        "panel-right",
        include_bytes!("../assets/icons/panel-right.svg"),
    ),
    ("pen", include_bytes!("../assets/icons/pen.svg")),
    ("plug", include_bytes!("../assets/icons/plug.svg")),
    ("plus", include_bytes!("../assets/icons/plus.svg")),
    ("refresh", include_bytes!("../assets/icons/refresh.svg")),
    ("search", include_bytes!("../assets/icons/search.svg")),
    ("shield", include_bytes!("../assets/icons/shield.svg")),
    ("slash", include_bytes!("../assets/icons/slash.svg")),
    ("spark", include_bytes!("../assets/icons/spark.svg")),
    ("star", include_bytes!("../assets/icons/star.svg")),
    ("star-fill", include_bytes!("../assets/icons/star-fill.svg")),
    ("stop", include_bytes!("../assets/icons/stop.svg")),
    ("sun", include_bytes!("../assets/icons/sun.svg")),
    ("terminal", include_bytes!("../assets/icons/terminal.svg")),
    ("trash", include_bytes!("../assets/icons/trash.svg")),
    ("up", include_bytes!("../assets/icons/up.svg")),
    ("x", include_bytes!("../assets/icons/x.svg")),
];
