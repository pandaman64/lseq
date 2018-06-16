use std::collections::BTreeMap;
use std::fmt;

use rand::prelude::*;

use key::{Id, InsertionStrategy, Key, INITIAL_WIDTH};

#[derive(Debug)]
pub struct Document<SiteId, Value> {
    content: BTreeMap<Key<SiteId>, Option<Value>>,
    strategies: Vec<InsertionStrategy>,
    clock: usize,
}

impl<SiteId: Ord + Clone + fmt::Debug, Value> Document<SiteId, Value> {
    pub fn new() -> Self {
        let mut content = BTreeMap::new();
        content.insert(
            Key {
                position: vec![(0, Id::Sentinel)],
                clock: 0,
            },
            None,
        );
        content.insert(
            Key {
                position: vec![(INITIAL_WIDTH, Id::Sentinel)],
                clock: 0,
            },
            None,
        );

        Document {
            content: content,
            strategies: vec![random()],
            clock: 2,
        }
    }

    pub fn start(&self) -> Key<SiteId> {
        Key {
            position: vec![(0, Id::Sentinel)],
            clock: 0,
        }
    }

    pub fn end(&self) -> Key<SiteId> {
        Key {
            position: vec![(INITIAL_WIDTH, Id::Sentinel)],
            clock: 0,
        }
    }

    pub fn insert(
        &mut self,
        site_id: SiteId,
        left: &Key<SiteId>,
        right: &Key<SiteId>,
        value: Value,
    ) -> Key<SiteId> {
        use std::collections::btree_map::Entry::*;

        let key = left.pick(right, Id::Site(site_id), self.clock, &mut self.strategies);
        assert!(
            left < &key && &key < right,
            "must hold {:?} < {:?} < {:?}",
            left,
            key,
            right
        );

        match self.content.entry(key.clone()) {
            Vacant(v) => {
                v.insert(Some(value));
            }
            Occupied(o) => {
                assert_eq!(&key, o.key());
                if o.key().clock < key.clock {
                    o.remove_entry();
                    self.content.insert(key.clone(), Some(value));
                }
            }
        }
        self.clock += 1;
        key
    }

    pub fn insert_at(&mut self, key: Key<SiteId>, value: Value) {
        self.content.insert(key, Some(value));
    }

    pub fn remove(&mut self, key: Key<SiteId>) {
        use std::collections::btree_map::Entry::*;
        let clock = key.clock;

        match self.content.entry(key) {
            Vacant(_) => {},
            Occupied(o) => {
                if o.key().clock == clock {
                    o.remove_entry();
                }
            }
        }
    }

    pub fn get(&self, key: &Key<SiteId>) -> Option<&Value> {
        self.content.get(key).and_then(Option::as_ref)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        let start = self.start();
        let end = self.end();
        self.content
            .iter()
            .filter(move |item| item.0 != &start && item.0 != &end)
            .map(|item| item.1.as_ref().unwrap())
    }

    pub fn len(&self) -> usize {
        self.content.len() - 2
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_equality() {
        let key = Key {
            position: vec![(1, Id::Site(())), (4, Id::Site(()))],
            clock: 0,
        };
        assert_eq!(key, key);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Characters {
        Alice,
        Bob,
    }

    #[test]
    fn test_fuzz() {
        use self::Characters::*;
        use std::collections::BTreeSet;

        let mut doc: Document<_, u8> = Document::new();

        let mut keys = BTreeSet::new();
        keys.insert(doc.start());
        keys.insert(doc.end());

        let mut result = vec![];

        let mut rng = thread_rng();

        for _ in 0..300 {
            // insertion
            if keys.len() <= 2 || rng.gen() {
                let new_key;
                let i;
                let value = rng.gen();

                // randomly pick adjacent keys
                {
                    let left;
                    let right;
                    {
                        i = rng.gen_range(0, keys.len() - 1);
                        let mut iter = keys.iter().skip(i);
                        left = iter.next().unwrap();
                        right = iter.next().unwrap();
                    }
                    new_key = doc.insert(if rng.gen() { Alice } else { Bob }, left, right, value);
                }

                keys.insert(new_key);
                result.insert(i, value);
            } else {
                // removal
                // randomly pick a key to remove
                let i;
                let key = {
                    i = rng.gen_range(1, keys.len() - 1);
                    keys.iter().skip(i).next().unwrap().clone()
                };

                keys.remove(&key);
                doc.remove(key);
                result.remove(i - 1);
            }
        }

        let mut correct = result.iter();
        let mut iter = doc.iter();
        loop {
            match (correct.next(), iter.next()) {
                (None, None) => break,
                (lhs, rhs) if lhs != rhs => panic!(),
                _ => {}
            }
        }
    }

    #[test]
    fn test_hello_world() {
        let mut doc = Document::new();
        let start = doc.start();
        let end = doc.end();

        let h = doc.insert((), &start, &end, "hello");
        let e = doc.insert((), &h, &end, "!");
        let _ = doc.insert((), &h, &e, "world");

        let mut iter = doc.iter();
        assert_eq!(iter.next(), Some(&"hello"));
        assert_eq!(iter.next(), Some(&"world"));
        assert_eq!(iter.next(), Some(&"!"));
    }
}
