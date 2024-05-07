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

struct OccupiedEntry<'a, K, V> {
    inner: StdOccupiedEntry<'a, K, V>,
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
    }

    #[test]
    fn just_insert_something() {
        let mut m = LimitedMap::<usize, String>::new();
        m.insert(1, "one".to_string());

        let entry = m.entry(1);
        let Entry::Occupied(entry) = entry else {
            panic!();
        };

        assert_eq!("one".to_string(), *entry.get());
    }
}
