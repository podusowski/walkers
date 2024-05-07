use std::collections::hash_map::Entry as StdEntry;
use std::collections::hash_map::OccupiedEntry as StdOccupiedEntry;
use std::collections::hash_map::VacantEntry as StdVacantEntry;
use std::collections::{HashMap, VecDeque};

struct LimitedMap<K, V> {
    pub values: std::collections::HashMap<K, V>,
    pub queue: std::collections::VecDeque<K>,
}

impl<K, V> LimitedMap<K, V>
where
    K: std::cmp::Eq + PartialEq + std::hash::Hash,
{
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn insert(&mut self, k: K, v: V) {
        self.values.insert(k, v);
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.values.entry(key) {
            StdEntry::Occupied(entry) => Entry::Occupied(entry),
            StdEntry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

enum Entry<'a, K, V> {
    Occupied(StdOccupiedEntry<'a, K, V>),
    Vacant(StdVacantEntry<'a, K, V>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacant_entry() {
        let mut m = LimitedMap::<usize, String>::new();

        let entry = m.entry(1);
        let Entry::Vacant(entry) = entry else {
            panic!();
        };

        entry.insert("one".to_string());

        assert_existence_with_entry_api(&mut m, 1, "one".to_string());
    }

    #[test]
    fn just_insert_something() {
        let mut m = LimitedMap::<usize, String>::new();
        m.insert(1, "one".to_string());

        assert_existence_with_entry_api(&mut m, 1, "one".to_string());
    }

    fn assert_existence_with_entry_api(
        m: &mut LimitedMap<usize, String>,
        key: usize,
        value: String,
    ) {
        let entry = m.entry(key);
        let Entry::Occupied(entry) = entry else {
            panic!();
        };

        assert_eq!(value, *entry.get());
    }
}
