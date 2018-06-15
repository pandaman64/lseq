#![feature(nll)]

use std::cmp;
use std::collections::BTreeMap;

extern crate rand;
use rand::distributions::Standard;
use rand::prelude::*;

const INITIAL_WIDTH: usize = 5;
const STEP: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Id<SiteId> {
    Sentinel,
    Site(SiteId),
}

#[derive(Clone, Debug, Hash)]
pub struct Key<SiteId> {
    position: Vec<(usize, Id<SiteId>)>,
    /// comparison of keys doesn't take clock into account
    clock: usize,
}

impl<SiteId: Ord> cmp::Ord for Key<SiteId> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        use cmp::Ordering::Equal;
        use Id::Sentinel;

        let max_level = self.position.len().max(other.position.len());

        let mut lhs = self.position.iter();
        let mut rhs = other.position.iter();

        for _ in 0..max_level {
            let left = lhs
                .next()
                .map(|item| (item.0, &item.1))
                .unwrap_or_else(|| (0, &Sentinel));
            let right = rhs
                .next()
                .map(|item| (item.0, &item.1))
                .unwrap_or_else(|| (0, &Sentinel));

            let cmp = left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1));
            if cmp != Equal {
                return cmp;
            }
        }

        Equal
    }
}

impl<SiteId: Ord> cmp::PartialOrd for Key<SiteId> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<SiteId: Eq> cmp::Eq for Key<SiteId> {}

impl<SiteId: PartialEq> cmp::PartialEq for Key<SiteId> {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position
    }
}

fn pick_index<R: Rng + ?Sized>(
    rng: &mut R,
    left: usize,
    right: usize,
    strategy: InsertionStrategy,
) -> usize {
    use InsertionStrategy::*;

    // [left, right) must have at least one element to pick
    assert!(left + 1 <= right);

    match strategy {
        Front => {
            let left = left;
            let right = left.saturating_add(STEP).min(right);

            rng.gen_range(left, right)
        }
        Back => {
            let right = right;
            let left = right.saturating_sub(STEP).max(left);

            rng.gen_range(left, right)
        }
    }
}

fn width(level: usize) -> usize {
    INITIAL_WIDTH * 2_usize.pow(level as u32)
}

fn get_strategy<R: Rng + ?Sized>(
    rng: &mut R,
    strategies: &mut Vec<InsertionStrategy>,
    level: usize,
) -> InsertionStrategy {
    for _ in strategies.len()..=level {
        strategies.push(rng.gen());
    }
    strategies[level]
}

impl<SiteId: Ord + Clone> Key<SiteId> {
    fn pick(
        &self,
        other: &Self,
        site_id: Id<SiteId>,
        clock: usize,
        strategies: &mut Vec<InsertionStrategy>,
    ) -> Self {
        assert!(*self < *other);

        let mut rng = rand::thread_rng();
        let mut lhs = self.position.iter();
        let mut rhs = other.position.iter();
        let mut ret = vec![];
        let mut level = 0;

        let pos = loop {
            let (lpos, lid) = lhs
                .next()
                .map(|x| (x.0, &x.1))
                .unwrap_or_else(|| (0, &Id::Sentinel));
            let (rpos, _) = rhs
                .next()
                .map(|x| (x.0, &x.1))
                .unwrap_or_else(|| (width(level), &Id::Sentinel));

            if lpos + 1 < rpos {
                let strategy = get_strategy(&mut rng, strategies, level);
                break pick_index(&mut rng, lpos + 1, rpos, strategy);
            } else if lpos + 1 == rpos && *lid < site_id {
                break lpos;
            } else {
                level += 1;
                ret.push((lpos, lid.clone()));
            }
        };
        ret.push((pos, site_id));

        Key {
            position: ret,
            clock,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum InsertionStrategy {
    Front,
    Back,
}

impl Distribution<InsertionStrategy> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> InsertionStrategy {
        let v: u8 = rng.gen_range(0, 2);
        match v {
            0 => InsertionStrategy::Front,
            1 => InsertionStrategy::Back,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct Document<SiteId, Value> {
    content: BTreeMap<Key<SiteId>, Option<Value>>,
    strategies: Vec<InsertionStrategy>,
    clock: usize,
}

impl<SiteId: Ord + Clone + std::fmt::Debug, Value> Document<SiteId, Value> {
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
            strategies: vec![rand::random()],
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
                let (old, _) = o.remove_entry();
                assert!(old.position == key.position && old.clock < key.clock);
                self.content.insert(key.clone(), Some(value));
            }
        }
        self.clock += 1;
        key
    }

    pub fn remove(&mut self, key: &Key<SiteId>) {
        assert!(self.content.remove(key).is_some());
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Value> {
        let start = self.start();
        let end = self.end();
        self.content
            .iter()
            .filter(move |item| item.0 != &start && item.0 != &end)
            .map(|item| item.1.as_ref().unwrap())
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

                doc.remove(&key);
                keys.remove(&key);
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
