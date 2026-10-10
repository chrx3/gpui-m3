//! Small per-thread LRU caches keyed by content, so parsed Markdown, diffs and highlighted code
//! are not rebuilt on every frame (GPUI renders run on the main thread).

use std::hash::{DefaultHasher, Hash, Hasher};

/// Hash of any number of strings (and their lengths) used as a cache key.
pub(crate) fn content_key(parts: &[&str]) -> u64 {
    let mut h = DefaultHasher::new();
    for part in parts {
        part.len().hash(&mut h);
        part.hash(&mut h);
    }
    h.finish()
}

/// Least-recently-used map with a fixed capacity. Lookups are linear, which is fine for the few
/// hundred entries these caches hold.
pub(crate) struct Lru<V> {
    cap: usize,
    entries: Vec<(u64, V)>,
}

impl<V: Clone> Lru<V> {
    pub(crate) const fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: Vec::new(),
        }
    }

    pub(crate) fn get(&mut self, key: u64) -> Option<V> {
        let i = self.entries.iter().position(|(k, _)| *k == key)?;
        let entry = self.entries.remove(i);
        let value = entry.1.clone();
        self.entries.push(entry);
        Some(value)
    }

    pub(crate) fn insert(&mut self, key: u64, value: V) {
        self.entries.retain(|(k, _)| *k != key);
        if self.entries.len() >= self.cap {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub(crate) fn get_or_insert_with(&mut self, key: u64, make: impl FnOnce() -> V) -> V {
        if let Some(value) = self.get(key) {
            return value;
        }
        let value = make();
        self.insert(key, value.clone());
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_the_least_recently_used() {
        let mut lru = Lru::new(2);
        lru.insert(1, "a");
        lru.insert(2, "b");
        assert_eq!(lru.get(1), Some("a"));
        lru.insert(3, "c");
        assert_eq!(lru.get(2), None);
        assert_eq!(lru.get(1), Some("a"));
        assert_eq!(lru.get(3), Some("c"));
    }

    #[test]
    fn keys_depend_on_part_boundaries() {
        assert_ne!(content_key(&["ab", "c"]), content_key(&["a", "bc"]));
        assert_eq!(content_key(&["x"]), content_key(&["x"]));
    }
}
