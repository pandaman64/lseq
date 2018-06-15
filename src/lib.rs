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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key<SiteId>(Vec<(usize, Id<SiteId>)>);

impl<SiteId: Ord> std::cmp::Ord for Key<SiteId> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        let max_level = self.0.len().max(other.0.len());

        let mut lhs = self.0.iter();
        let mut rhs = other.0.iter();

        for _ in 0..max_level {
            let left = lhs
                .next()
                .map(|item| (item.0, &item.1))
                .unwrap_or_else(|| (0, &Id::Sentinel));
            let right = rhs
                .next()
                .map(|item| (item.0, &item.1))
                .unwrap_or_else(|| (0, &Id::Sentinel));

            let cmp = left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1));
            if cmp != Equal {
                return cmp;
            }
        }

        Equal
    }
}

impl<SiteId: Ord> std::cmp::PartialOrd for Key<SiteId> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
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
        strategies: &mut Vec<InsertionStrategy>,
    ) -> Self {
        assert!(*self < *other);

        let mut rng = rand::thread_rng();
        let mut lhs = self.0.iter();
        let mut rhs = other.0.iter();
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
        Key(ret)
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
    current_level: usize,
}

impl<SiteId: Ord + Clone + std::fmt::Debug, Value> Document<SiteId, Value> {
    pub fn new() -> Self {
        let mut content = BTreeMap::new();
        content.insert(Key(vec![(0, Id::Sentinel)]), None);
        content.insert(Key(vec![(INITIAL_WIDTH, Id::Sentinel)]), None);

        Document {
            content: content,
            strategies: vec![rand::random()],
            current_level: 0,
        }
    }

    pub fn start(&self) -> Key<SiteId> {
        Key(vec![(0, Id::Sentinel)])
    }

    pub fn end(&self) -> Key<SiteId> {
        Key(vec![(INITIAL_WIDTH, Id::Sentinel)])
    }

    pub fn insert(
        &mut self,
        site_id: SiteId,
        left: &Key<SiteId>,
        right: &Key<SiteId>,
        value: Value,
    ) -> Key<SiteId> {
        let key = left.pick(right, Id::Site(site_id), &mut self.strategies);
        assert!(
            left < &key && &key < right,
            "must hold {:?} < {:?} < {:?}",
            left,
            key,
            right
        );
        assert!(
            self.content.insert(key.clone(), Some(value)).is_none(),
            "key collided: {:?}",
            key
        );
        key
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
        let key = Key(vec![(1, Id::Site(())), (4, Id::Site(()))]);
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

        let mut rng = thread_rng();

        for _ in 0..100 {
            let new_key;

            // randomly pick adjacent keys
            {
                let left;
                let right;
                {
                    let i = rng.gen_range(0, keys.len() - 1);
                    let mut iter = keys.iter().skip(i);
                    left = iter.next().unwrap();
                    right = iter.next().unwrap();
                }
                new_key = doc.insert(if rng.gen() { Alice } else { Bob }, left, right, rng.gen());
            }

            keys.insert(new_key);
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
