//! A small seeded generator (SplitMix64). The same seed always gives the same numbers, on every
//! machine, so a fuzz run can be repeated from the seed it printed.

#[derive(Debug, Clone)]
pub(crate) struct Rng(u64);

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// A number in `0..n`; `0` when `n` is `0`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// A number in `lo..=hi`; `lo` when the range is empty.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = u64::try_from(i128::from(hi) - i128::from(lo) + 1).unwrap_or(u64::MAX);
        let off = i128::from(self.below(span));
        i64::try_from(i128::from(lo) + off).unwrap_or(lo)
    }

    /// True in `percent` of the calls.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// An index in `0..len` (`0` for an empty slice).
    pub fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.below(len as u64)).unwrap_or(0)
    }
}

/// A seed for one endpoint: the run's seed mixed with a name, so each endpoint has its own
/// stream and adding an endpoint does not change the requests of the others.
pub(crate) fn derive(seed: u64, key: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in key.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    mix(seed ^ mix(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        let xs: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let ys: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(xs, ys);
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn range_stays_inside_its_bounds() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let n = r.range(-3, 4);
            assert!((-3..=4).contains(&n));
        }
        assert_eq!(r.range(5, 5), 5);
        // The widest range must not overflow.
        let _ = r.range(i64::MIN, i64::MAX);
    }

    #[test]
    fn endpoints_get_their_own_streams() {
        assert_ne!(derive(1, "A.x"), derive(1, "A.y"));
        assert_eq!(derive(1, "A.x"), derive(1, "A.x"));
    }
}
