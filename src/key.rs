use rand::distributions::Standard;
use rand::prelude::*;
use std::cmp;

pub(crate) const INITIAL_WIDTH: usize = 5;
const STEP: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Id<SiteId> {
    Sentinel,
    Site(SiteId),
}

#[derive(Clone, Debug, Hash)]
pub struct Key<SiteId> {
    pub(crate) position: Vec<(usize, Id<SiteId>)>,
    /// comparison of keys doesn't take clock into account
    pub(crate) clock: usize,
}

impl<SiteId: Ord> cmp::Ord for Key<SiteId> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        use self::Id::Sentinel;
        use std::cmp::Ordering::Equal;

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

fn pick_position<R: Rng + ?Sized>(
    rng: &mut R,
    left: usize,
    right: usize,
    strategy: InsertionStrategy,
) -> usize {
    use self::InsertionStrategy::*;

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
    pub(crate) fn pick(
        &self,
        other: &Self,
        site_id: Id<SiteId>,
        clock: usize,
        strategies: &mut Vec<InsertionStrategy>,
    ) -> Self {
        assert!(*self < *other);

        let mut rng = thread_rng();
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
                break pick_position(&mut rng, lpos + 1, rpos, strategy);
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
pub(crate) enum InsertionStrategy {
    Front,
    Back,
}

impl Distribution<InsertionStrategy> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> InsertionStrategy {
        use self::InsertionStrategy::*;

        let v: u8 = rng.gen_range(0, 2);
        match v {
            0 => Front,
            1 => Back,
            _ => unreachable!(),
        }
    }
}
