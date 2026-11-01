//! What Gscouter asks of a simulator: its `score.RunIterations`, iterations
//! `first` to `first + count - 1` of a config, each reduced to the part of
//! the fight a score needs (`score.Iteration`), as JSON in the same form.
//!
//! The reduction follows Gscouter's `summarize` (internal/score/score.go):
//! the window starts at the 0.5 s bucket the warmup falls in and ends with
//! the fight; its halves split at a bucket boundary; waits are seconds in
//! the window by gcsim's reason names.

use std::fmt::Write as _;

use crate::types::{FAILURE_NAMES, N_FAILURES};
use crate::{check, eval, gcsl, profile, sim};

/// gcsim's damage bucket, 0.5 s.
const BUCKET: i32 = 30;

/// One fight, as Gscouter's `score.Iteration`.
#[derive(Debug, Clone, PartialEq)]
pub struct Iteration {
    /// Team DPS over the window.
    pub dps: f64,
    /// DPS in the first and the second half of the window.
    pub first: f64,
    pub second: f64,
    /// DPS over the window by character, in config order.
    pub chars: Vec<f64>,
    /// Seconds within the window that actions waited, by character and
    /// reason; reasons with none are left out.
    pub waits: Vec<Vec<(&'static str, f64)>>,
    /// The fight's length in frames.
    pub frames: i32,
    /// gcsim's character keys, in config order.
    pub names: Vec<&'static str>,
}

/// Runs iterations `first` to `first + count - 1` of the config `src`;
/// iteration i uses seed `seed + i`, as Gscouter's does. A config that
/// needs something the engine does not implement is an error that names
/// it, before anything runs.
pub fn run_iterations(src: &str, warmup: f64, seed: i64, first: usize, count: usize) -> Result<Vec<Iteration>, String> {
    if count == 0 {
        return Err("iterations must be positive".into());
    }
    let parsed = gcsl::parse(src)?;
    let missing = check::unsupported(&parsed);
    if !missing.is_empty() {
        return Err(format!("unsupported: {}", missing.join(", ")));
    }
    // A config without a duration runs until its action list ends.
    if parsed.duration > 0.0 && parsed.duration <= warmup {
        return Err(format!("duration {}s must be longer than warmup {}s", parsed.duration, warmup));
    }
    let cfg = profile::config(&parsed).map_err(|e| e.join("; "))?;
    let names: Vec<&'static str> = parsed.chars.iter().map(|c| gcsl::char_name(c.key)).collect();
    let mut out = Vec::with_capacity(count);
    for j in 0..count {
        let i = first + j;
        let s = seed.wrapping_add(i as i64);
        let mut sim = sim::Sim::new(&cfg, s as u64, warmup);
        eval::run(&mut sim, &parsed.program).map_err(|e| format!("iteration {i} (seed {s}): {e}"))?;
        sim.close_waits();
        out.push(summarize(&sim, &names));
    }
    Ok(out)
}

/// Gscouter's `summarize` of one fight.
fn summarize(sim: &sim::Sim, names: &[&'static str]) -> Iteration {
    let n = names.len();
    let start = sim.window_start;
    let end = sim.f;
    let mut it = Iteration { dps: 0.0, first: 0.0, second: 0.0, chars: vec![0.0; n], waits: vec![Vec::new(); n], frames: end, names: names.to_vec() };
    let window = (end - start) as f64 / 60.0;
    if window <= 0.0 {
        return it;
    }
    // buckets are counted from the window's start, so the middle one is
    // taken relative to it as Gscouter takes it from the fight's start
    let start_bucket = start / BUCKET;
    let mid_bucket = (start + end) / 2 / BUCKET;
    let mid_frame = mid_bucket * BUCKET;
    for (b, d) in sim.window_buckets.iter().enumerate() {
        if start_bucket + b as i32 >= mid_bucket {
            it.second += d;
        } else {
            it.first += d;
        }
    }
    it.dps = (it.first + it.second) / window;
    if mid_frame > start {
        it.first /= (mid_frame - start) as f64 / 60.0;
        it.second /= (end - mid_frame) as f64 / 60.0;
    } else {
        it.first = it.dps;
        it.second = it.dps;
    }
    for c in 0..n {
        it.chars[c] = sim.window_char[c] / window;
        for k in 0..N_FAILURES {
            let frames = sim.waits[c][k];
            if frames > 0 {
                it.waits[c].push((FAILURE_NAMES[k], frames as f64 / 60.0));
            }
        }
    }
    it
}

/// The iterations as Gscouter's JSON (`[]score.Iteration`).
pub fn to_json(its: &[Iteration]) -> String {
    let mut o = String::with_capacity(256 * its.len() + 2);
    o.push('[');
    for (i, it) in its.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{{\"dps\":{},\"first\":{},\"second\":{},\"chars\":[", num(it.dps), num(it.first), num(it.second));
        for (c, d) in it.chars.iter().enumerate() {
            if c > 0 {
                o.push(',');
            }
            o.push_str(&num(*d));
        }
        o.push_str("],\"waits\":[");
        for (c, w) in it.waits.iter().enumerate() {
            if c > 0 {
                o.push(',');
            }
            o.push('{');
            for (k, (reason, secs)) in w.iter().enumerate() {
                if k > 0 {
                    o.push(',');
                }
                let _ = write!(o, "\"{reason}\":{}", num(*secs));
            }
            o.push('}');
        }
        let _ = write!(o, "],\"frames\":{},\"names\":[", it.frames);
        for (c, name) in it.names.iter().enumerate() {
            if c > 0 {
                o.push(',');
            }
            let _ = write!(o, "\"{name}\"");
        }
        o.push_str("]}");
    }
    o.push(']');
    o
}

/// A number as JSON: Rust's shortest form, which reads back to the same
/// f64. JSON has no NaN or infinity, and a fight gives neither; should one
/// appear, it is written as 0 rather than as invalid JSON.
fn num(x: f64) -> String {
    if x.is_finite() { format!("{x}") } else { "0".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../configs/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn halves_and_chars_add_up() {
        let its = run_iterations(&config("team-looped.txt"), 150.0, 1, 0, 3).unwrap();
        for it in &its {
            let chars: f64 = it.chars.iter().sum();
            assert!((chars - it.dps).abs() < 1e-6 * it.dps, "{chars} {}", it.dps);
            // equal halves of the window: their mean is the window's DPS
            assert!(((it.first + it.second) / 2.0 - it.dps).abs() < 1e-6 * it.dps);
            assert_eq!(it.frames, 27000);
            assert_eq!(it.names, ["hutao", "xingqiu", "zhongli", "yelan"]);
        }
    }

    #[test]
    fn a_slice_is_the_same_iterations() {
        let src = config("team-looped.txt");
        let all = run_iterations(&src, 150.0, 7, 0, 4).unwrap();
        let tail = run_iterations(&src, 150.0, 7, 2, 2).unwrap();
        assert_eq!(all[2..], tail[..]);
    }

    #[test]
    fn one_pass_has_a_window_from_the_start() {
        let its = run_iterations(&config("team-onepass.txt"), 0.0, 1, 0, 2).unwrap();
        for it in &its {
            assert!(it.dps > 0.0 && it.frames > 0);
        }
    }

    #[test]
    fn errors() {
        let src = config("team-looped.txt");
        assert!(run_iterations(&src, 150.0, 1, 0, 0).is_err());
        assert!(run_iterations(&src, 450.0, 1, 0, 1).unwrap_err().contains("longer than warmup"));
        let bad = src.replace("hutao char", "nosuchchar char");
        assert!(run_iterations(&bad, 150.0, 1, 0, 1).is_err());
    }

    #[test]
    fn json_shape() {
        let it = Iteration { dps: 1.5, first: 1.0, second: 2.0, chars: vec![1.5, 0.0], waits: vec![vec![("insufficient_energy", 0.25)], vec![]], frames: 600, names: vec!["hutao", "xingqiu"] };
        assert_eq!(
            to_json(&[it]),
            r#"[{"dps":1.5,"first":1,"second":2,"chars":[1.5,0],"waits":[{"insufficient_energy":0.25},{}],"frames":600,"names":["hutao","xingqiu"]}]"#
        );
    }
}
