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

/// Least-recently-used map with a fixed capacity. Lookups are hashed and each use only bumps a
/// counter, so a frame that reads every message of two long conversations stays cheap; when the
/// map is full the oldest quarter goes at once.
pub(crate) struct Lru<V> {
    cap: usize,
    tick: u64,
    entries: Option<std::collections::HashMap<u64, (V, u64)>>,
}

impl<V: Clone> Lru<V> {
    pub(crate) const fn new(cap: usize) -> Self {
        Self {
            cap,
            tick: 0,
            entries: None,
        }
    }

    pub(crate) fn get(&mut self, key: u64) -> Option<V> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.entries.as_mut()?.get_mut(&key)?;
        entry.1 = tick;
        Some(entry.0.clone())
    }

    pub(crate) fn insert(&mut self, key: u64, value: V) {
        self.tick += 1;
        let tick = self.tick;
        let cap = self.cap;
        let entries = self.entries.get_or_insert_with(Default::default);
        if !entries.contains_key(&key) && entries.len() >= cap {
            let mut ages: Vec<u64> = entries.values().map(|(_, used)| *used).collect();
            let drop = (cap / 4).max(1).min(ages.len());
            ages.select_nth_unstable(drop - 1);
            let cutoff = ages[drop - 1];
            entries.retain(|_, (_, used)| *used > cutoff);
        }
        entries.insert(key, (value, tick));
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
    fn a_full_map_drops_its_oldest_quarter() {
        let mut lru = Lru::new(8);
        for key in 0..8 {
            lru.insert(key, key);
        }
        // 0 and 1 are used again, so 2 and 3 become the oldest.
        lru.get(0);
        lru.get(1);
        lru.insert(8, 8);
        assert_eq!(lru.get(2), None);
        assert_eq!(lru.get(3), None);
        assert_eq!(lru.get(0), Some(0));
        assert_eq!(lru.get(8), Some(8));
    }

    #[test]
    fn keys_depend_on_part_boundaries() {
        assert_ne!(content_key(&["ab", "c"]), content_key(&["a", "bc"]));
        assert_eq!(content_key(&["x"]), content_key(&["x"]));
    }
}
