use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Map which deletes old entries when a limit is hit.
pub struct LimitedMap<K, V> {
    values: std::collections::HashMap<K, V>,
    queue: std::collections::VecDeque<K>,
    limit: usize,
}

impl<K, V> LimitedMap<K, V>
where
    K: Eq + PartialEq + Hash + Clone,
{
    /// Create a new LimitedMap with a limit.
    pub fn new(limit: usize) -> Self {
        Self {
            values: HashMap::new(),
            queue: VecDeque::new(),
            limit,
        }
    }

    /// Insert a key-value pair into the map. If the map is at its limit, the oldest entry will be
    /// removed.
    pub fn insert(&mut self, k: K, v: V) {
        if self.values.len() == self.limit {
            if let Some(front) = self.queue.pop_front() {
                self.values.remove(&front);
            }
        }

        self.values.insert(k.clone(), v);
        self.queue.push_back(k);
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        self.values.get(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_insert_something() {
        let mut m = LimitedMap::<usize, String>::new(1);
        m.insert(1, "one".to_string());
        assert_eq!(m.get(&1), Some(&"one".to_string()));
    }

    #[test]
    fn try_to_insert_something_above_limit() {
        let mut m = LimitedMap::<usize, String>::new(1);
        m.insert(1, "one".to_string());
        m.insert(2, "two".to_string());

        assert_eq!(m.get(&2), Some(&"two".to_string()));

        // First gets swept.
        assert_eq!(m.get(&1), None);
    }
}
