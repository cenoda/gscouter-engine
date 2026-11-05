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
        // each names itself as unsupported
        return Err(missing.join("; "));
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

/// What the config needs that the engine does not implement, as `gse
/// --check` lists it: empty when the engine can run it. A config that does
/// not parse is an error.
pub fn unsupported(src: &str) -> Result<Vec<String>, String> {
    match gcsl::parse(src) {
        Ok(p) => Ok(check::unsupported(&p)),
        Err(e) if e.starts_with("unsupported: ") => Ok(vec![e]),
        Err(e) => Err(e),
    }
}

/// The key catalog as versioned JSON (`catalog.rs`): characters, weapons
/// and sets with their keys, aliases and data, as `gse --catalog` prints it.
pub fn catalog() -> String {
    crate::catalog::json()
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

/// A string as JSON.
pub fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
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
        // a character gcsim has and the engine does not port yet, whichever
        // that is when the test runs
        // (not an alias of a ported one: "amy" is Fischl's)
        let ported = |id: &_| crate::data::CHAR_NAMES.iter().any(|(n, i)| i == id && crate::profile::char_kind(n).is_some());
        let (name, _) = crate::data::CHAR_NAMES.iter().find(|(n, id)| crate::profile::char_kind(n).is_none() && !ported(id)).expect("a character not ported");
        let bad = src.replace("hutao char", &format!("{name} char")).replace("hutao add", &format!("{name} add"));
        let want = format!("unsupported: character {name}");
        assert_eq!(unsupported(&bad).unwrap(), [want.clone()]);
        assert_eq!(run_iterations(&bad, 150.0, 1, 0, 1).unwrap_err(), want);
        assert!(unsupported(&src).unwrap().is_empty());

        let two_keys = "target lvl=100 resist=0.1 radius=2 pos=0,2.4;\nhutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add weapon=\"keyofkhajnisut\" refine=1 lvl=90/90;\nxingqiu char lvl=90/90 cons=0 talent=9,9,9;\nxingqiu add weapon=\"keyofkhajnisut\" refine=1 lvl=90/90;\nactive hutao;\nhutao attack;\n";
        assert_eq!(unsupported(two_keys).unwrap(), ["unsupported: weapon keyofkhajnisut on two characters"]);

        let two_verm = "target lvl=100 resist=0.1 radius=2 pos=0,2.4;\nhutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add weapon=\"whitetassel\" refine=1 lvl=90/90;\nhutao add set=\"vermillionhereafter\" count=4;\nxingqiu char lvl=90/90 cons=0 talent=9,9,9;\nxingqiu add weapon=\"travelershandysword\" refine=1 lvl=70/70;\nxingqiu add set=\"vermillionhereafter\" count=4;\nactive hutao;\nhutao attack;\n";
        assert_eq!(unsupported(two_verm).unwrap(), ["unsupported: set vermillionhereafter 4-piece on two characters"]);

        let dahlia_low = "target lvl=100 resist=0.1 radius=2 pos=0,2.4;\ndahlia char lvl=60/60 cons=0 talent=6,6,6;\ndahlia add weapon=\"travelershandysword\" refine=1 lvl=70/70;\nactive dahlia;\ndahlia burst;\n";
        assert_eq!(unsupported(dahlia_low).unwrap(), ["unsupported: dahlia's burst below ascension 4"]);

        let dori_charge = "target lvl=100 resist=0.1 radius=2 pos=0,2.4;\ndori char lvl=90/90 cons=0 talent=9,9,9;\ndori add weapon=\"thebell\" refine=1 lvl=70/70;\nactive dori;\ndori charge;\n";
        assert_eq!(unsupported(dori_charge).unwrap(), ["unsupported: action dori charge"]);
    }

    /// team-looped.txt's characters with other conditions and program.
    fn with_program(options: &str, target: &str, program: &str) -> String {
        let src = config("team-looped.txt");
        let build: Vec<&str> = src.lines().filter(|l| l.contains(" char ") || l.contains(" add ")).collect();
        format!("{options}\ntarget lvl=100 resist=0.1 radius=2 pos=0,2.4 {target};\n{}\nactive hutao;\n{program}\n", build.join("\n"))
    }

    #[test]
    fn stuck_on_one_frame() {
        let src = with_program("options swap_delay=12 duration=450;", "", "while 1 { wait(0); }");
        let e = run_iterations(&src, 0.0, 1, 0, 1).unwrap_err();
        assert!(e.ends_with(crate::sim::STUCK), "{e}");
        let src = with_program("options swap_delay=12 duration=450;", "", "hutao attack; while 1 { delay(0); }");
        assert!(run_iterations(&src, 0.0, 1, 0, 1).unwrap_err().ends_with(crate::sim::STUCK));
    }

    #[test]
    fn longest_fight() {
        // no duration and an enemy that takes no damage: the rotation never
        // ends, and the fight stops at Gscouter's limit
        let src = with_program("options swap_delay=12;", "hp=999999999", "while 1 { wait(600); }");
        let its = run_iterations(&src, 0.0, 1, 0, 1).unwrap();
        assert_eq!(its[0].frames, crate::sim::MAX_FIGHT_FRAMES);
        assert_eq!(its[0].dps, 0.0);
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
