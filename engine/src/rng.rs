//! xoshiro256++, seeded through splitmix64. The engine does not reproduce
//! gcsim's random numbers, only their distributions.

#[derive(Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        };
        Rng { s: [next(), next(), next(), next()] }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let r = (s[0].wrapping_add(s[3])).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }

    /// A float in [0, 1), as Go's rand.Float64.
    #[inline]
    pub fn float64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A standard normal sample, as Go's rand.NormFloat64 (Box-Muller here).
    pub fn norm_float64(&mut self) -> f64 {
        let u = 1.0 - self.float64();
        let v = self.float64();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// An int in [0, n), as Go's rand.Intn.
    #[inline]
    pub fn intn(&mut self, n: i32) -> i32 {
        ((self.next_u64() >> 1) % n as u64) as i32
    }
}
