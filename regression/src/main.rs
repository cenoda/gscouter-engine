//! `regress`: the engine's own library regression (docs/regression.md).
//!
//!     regress run --gse <gse> --configs <dir> [--iter 50] [--seed 1] [--warmup 150]
//!                 [--jobs N] [--shard k/n] [--engine <commit>] [--gscouter <commit>] [-o <file>]
//!     regress diff <baseline> <results>... [--shard k/n]
//!     regress merge <results>... [-o <file>]
//!
//! `run` runs every config of the directory (`<id>.txt`) on the engine alone,
//! one thread each, the same seeds every time, and writes one line per build:
//! team and character DPS and waits as `gse` prints them, and a hash of the
//! engine's whole output (its timing left out). `diff` compares results with
//! a baseline, prints which builds moved and by how much, and exits with 1
//! on any difference. `merge` joins the results of shards into one file, the
//! form of the committed baseline.

mod sha256;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use sha256::Sha256;

const FORMAT: &str = "1";
/// Header keys that must agree for two files to be comparable.
const PARAMS: [&str; 5] = ["format", "iterations", "warmup", "threads", "seed"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("diff") => diff(&args[1..]),
        Some("merge") => merge(&args[1..]),
        _ => Err("usage: regress run|diff|merge ... (see regression/src/main.rs)".into()),
    };
    match r {
        Ok(code) => code,
        Err(e) => {
            eprintln!("regress: {e}");
            ExitCode::from(2)
        }
    }
}

/// Options `--name value` and the remaining positional arguments.
fn opts(args: &[String]) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let mut o = BTreeMap::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with('-') && a.len() > 1 {
            let v = args.get(i + 1).ok_or_else(|| format!("{a} needs a value"))?;
            o.insert(a.trim_start_matches('-').to_string(), v.clone());
            i += 2;
        } else {
            pos.push(a.clone());
            i += 1;
        }
    }
    Ok((o, pos))
}

fn shard(o: &BTreeMap<String, String>) -> Result<Option<(u64, u64)>, String> {
    let Some(s) = o.get("shard") else { return Ok(None) };
    let (k, n) = s.split_once('/').ok_or("--shard is k/n")?;
    let (k, n): (u64, u64) = (k.parse().map_err(|_| "--shard is k/n")?, n.parse().map_err(|_| "--shard is k/n")?);
    if n == 0 || k >= n {
        return Err("--shard k/n needs 0 <= k < n".into());
    }
    Ok(Some((k, n)))
}

/// A build's shard: FNV-1a of its id, so it does not depend on the others.
fn in_shard(id: &str, s: Option<(u64, u64)>) -> bool {
    let Some((k, n)) = s else { return true };
    let mut h: u64 = 0xcbf29ce484222325;
    for b in id.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h % n == k
}

/// One build's result: `fields` in the order written, `sha` the hash.
#[derive(Clone, PartialEq)]
struct Rec {
    sha: String,
    team: Option<String>,
    chars: Vec<(String, String)>,
    waits: Vec<(String, String)>,
    frames: Option<String>,
    error: Option<String>,
}

impl Rec {
    fn line(&self, id: &str) -> String {
        let mut s = format!("{id} sha={}", self.sha);
        if let Some(t) = &self.team {
            write!(s, " team={t}").unwrap();
        }
        for (c, v) in &self.chars {
            write!(s, " {c}={v}").unwrap();
        }
        for (w, v) in &self.waits {
            write!(s, " wait:{w}={v}").unwrap();
        }
        if let Some(f) = &self.frames {
            write!(s, " frames={f}").unwrap();
        }
        if let Some(e) = &self.error {
            write!(s, " error={e}").unwrap();
        }
        s
    }

    fn parse(line: &str) -> Result<(String, Rec), String> {
        let (id, rest) = line.split_once(' ').ok_or_else(|| format!("bad line: {line}"))?;
        let mut r = Rec { sha: String::new(), team: None, chars: vec![], waits: vec![], frames: None, error: None };
        let (rest, err) = match rest.find(" error=") {
            Some(i) => (&rest[..i], Some(rest[i + 7..].to_string())),
            None => match rest.strip_prefix("error=") {
                Some(e) => ("", Some(e.to_string())),
                None => (rest, None),
            },
        };
        r.error = err;
        for tok in rest.split(' ').filter(|t| !t.is_empty()) {
            let (k, v) = tok.split_once('=').ok_or_else(|| format!("bad field {tok} in: {line}"))?;
            match k {
                "sha" => r.sha = v.into(),
                "team" => r.team = Some(v.into()),
                "frames" => r.frames = Some(v.into()),
                _ => match k.strip_prefix("wait:") {
                    Some(w) => r.waits.push((w.into(), v.into())),
                    None => r.chars.push((k.into(), v.into())),
                },
            }
        }
        if r.sha.is_empty() {
            return Err(format!("no sha in: {line}"));
        }
        Ok((id.to_string(), r))
    }
}

/// Reads gse's printed output: the team and character means, the waits and
/// the frames (the abilities reach only the hash).
fn read_output(out: &str) -> Rec {
    let mut r = Rec { sha: String::new(), team: None, chars: vec![], waits: vec![], frames: None, error: None };
    let mut section = "";
    for l in out.lines() {
        if let Some(rest) = l.strip_prefix("  \"team\": {\"mean\": ") {
            r.team = rest.split(',').next().map(str::to_string);
        } else if let Some(rest) = l.strip_prefix("  \"frames\": ") {
            r.frames = Some(rest.trim_end_matches(',').to_string());
        } else if let Some(rest) = l.strip_prefix("  \"") {
            section = rest.split('"').next().unwrap_or("");
        } else if let Some(rest) = l.strip_prefix("    \"") {
            let (name, rest) = rest.split_once("\": ").unwrap_or((rest, ""));
            match section {
                "chars" => {
                    let m = rest.strip_prefix("{\"mean\": ").and_then(|s| s.split(',').next()).unwrap_or("?");
                    r.chars.push((name.to_string(), m.to_string()));
                }
                "waits" => {
                    let inner = rest.trim_end_matches(',').trim_start_matches('{').trim_end_matches('}');
                    for kv in inner.split(", ").filter(|s| !s.is_empty()) {
                        if let Some((k, v)) = kv.split_once(": ") {
                            r.waits.push((format!("{name}/{}", k.trim_matches('"')), v.to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    r
}

fn run_one(gse: &str, path: &Path, iter: &str, warmup: &str, seed: &str) -> Rec {
    let out = Command::new(gse).arg(path).args([iter, warmup, "1", "--seed", seed]).output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            return Rec { sha: "-".into(), team: None, chars: vec![], waits: vec![], frames: None, error: Some(format!("cannot run gse: {e}")) };
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut h = Sha256::new();
    for l in stdout.lines().filter(|l| !l.starts_with("  \"seconds\": ")) {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    h.update(format!("\0status={:?}\0", out.status.code()).as_bytes());
    h.update(stderr.as_bytes());
    let mut r = if out.status.success() { read_output(&stdout) } else { Rec { sha: String::new(), team: None, chars: vec![], waits: vec![], frames: None, error: None } };
    if !out.status.success() {
        let msg = stderr.lines().next().unwrap_or("").trim();
        r.error = Some(format!("{} {}", out.status.code().map_or("signal".into(), |c| c.to_string()), msg.strip_prefix("error: ").unwrap_or(msg)));
    }
    r.sha = h.hex()[..32].to_string();
    r
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let (o, _) = opts(args)?;
    let gse = o.get("gse").ok_or("--gse is needed")?.clone();
    let dir = PathBuf::from(o.get("configs").ok_or("--configs is needed")?);
    let get = |k: &str, d: &str| o.get(k).cloned().unwrap_or_else(|| d.to_string());
    let (iter, seed, warmup) = (get("iter", "50"), get("seed", "1"), get("warmup", "150"));
    let sh = shard(&o)?;
    let jobs: usize = match o.get("jobs") {
        Some(j) => j.parse().map_err(|_| "--jobs is a number")?,
        None => std::thread::available_parallelism().map_or(1, |n| n.get()),
    };

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".txt"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("no configs in {}", dir.display()));
    }
    // the inputs' hash covers every config, so the shards of one run agree
    let mut ih = Sha256::new();
    for n in &names {
        let b = std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"))?;
        ih.update(n.as_bytes());
        ih.update(b"\0");
        ih.update(&b);
        ih.update(b"\0");
    }
    let inputs = ih.hex();
    let ids: Vec<String> = names.iter().map(|n| n.trim_end_matches(".txt").to_string()).filter(|id| in_shard(id, sh)).collect();

    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let results: Mutex<BTreeMap<String, Rec>> = Mutex::new(BTreeMap::new());
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(id) = ids.get(i) else { break };
                let r = run_one(&gse, &dir.join(format!("{id}.txt")), &iter, &warmup, &seed);
                results.lock().unwrap().insert(id.clone(), r);
            });
        }
    });
    let results = results.into_inner().unwrap();

    let mut out = String::new();
    writeln!(out, "# gse-regress").unwrap();
    for (k, v) in [
        ("format", FORMAT.to_string()),
        ("engine", get("engine", "unknown")),
        ("gscouter", get("gscouter", "unknown")),
        ("inputs", inputs),
        ("configs", names.len().to_string()),
        ("iterations", iter),
        ("warmup", warmup),
        ("threads", "1".into()),
        ("seed", seed),
    ] {
        writeln!(out, "# {k} {v}").unwrap();
    }
    if let Some((k, n)) = sh {
        writeln!(out, "# shard {k}/{n}").unwrap();
    }
    for (id, r) in &results {
        writeln!(out, "{}", r.line(id)).unwrap();
    }
    write_out(o.get("o"), &out)?;
    eprintln!("regress: {} builds in {:.1} s on {} jobs", results.len(), start.elapsed().as_secs_f64(), jobs);
    Ok(ExitCode::SUCCESS)
}

fn write_out(path: Option<&String>, s: &str) -> Result<(), String> {
    match path {
        Some(p) => std::fs::write(p, s).map_err(|e| format!("{p}: {e}")),
        None => {
            print!("{s}");
            Ok(())
        }
    }
}

struct File {
    head: BTreeMap<String, String>,
    recs: BTreeMap<String, Rec>,
}

fn load(paths: &[String]) -> Result<File, String> {
    let mut f = File { head: BTreeMap::new(), recs: BTreeMap::new() };
    for p in paths {
        let s = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        for l in s.lines() {
            if let Some(h) = l.strip_prefix("# ") {
                let (k, v) = h.split_once(' ').unwrap_or((h, ""));
                if k == "gse-regress" || k == "shard" || v.is_empty() {
                    continue;
                }
                if let Some(old) = f.head.insert(k.into(), v.into()) {
                    if old != v {
                        return Err(format!("{p}: header {k} is {v}, another file has {old}"));
                    }
                }
            } else if !l.trim().is_empty() {
                let (id, r) = Rec::parse(l)?;
                if f.recs.insert(id.clone(), r).is_some() {
                    return Err(format!("{p}: {id} twice"));
                }
            }
        }
    }
    Ok(f)
}

fn merge(args: &[String]) -> Result<ExitCode, String> {
    let (o, pos) = opts(args)?;
    let f = load(&pos)?;
    let mut out = String::from("# gse-regress\n");
    for k in ["format", "engine", "gscouter", "inputs", "configs", "iterations", "warmup", "threads", "seed"] {
        if let Some(v) = f.head.get(k) {
            writeln!(out, "# {k} {v}").unwrap();
        }
    }
    if let Some(n) = f.head.get("configs").and_then(|n| n.parse::<usize>().ok()) {
        if n != f.recs.len() {
            return Err(format!("the results have {} builds of {n} configs: a shard is missing", f.recs.len()));
        }
    }
    for (id, r) in &f.recs {
        writeln!(out, "{}", r.line(id)).unwrap();
    }
    write_out(o.get("o"), &out)?;
    Ok(ExitCode::SUCCESS)
}

fn num(s: &str) -> Option<f64> {
    s.parse().ok()
}

fn pct(a: f64, b: f64) -> String {
    if a.abs() < 1e-9 {
        if b.abs() < 1e-9 { "0".into() } else { "new".into() }
    } else {
        format!("{:+.3}%", (b - a) / a * 100.0)
    }
}

fn diff(args: &[String]) -> Result<ExitCode, String> {
    let (o, pos) = opts(args)?;
    if pos.len() < 2 {
        return Err("diff needs a baseline and results".into());
    }
    let sh = shard(&o)?;
    let base = load(&pos[..1])?;
    let new = load(&pos[1..])?;
    let mut out = String::new();
    let mut differs = false;

    writeln!(out, "## Regression against the baseline\n").unwrap();
    for k in ["engine", "gscouter", "inputs"] {
        let (a, b) = (base.head.get(k).map_or("?", |s| s), new.head.get(k).map_or("?", |s| s));
        writeln!(out, "- {k}: baseline `{a}`, results `{b}`{}", if a != b && k == "inputs" { " (**the inputs differ**: regenerate the baseline with them)" } else { "" }).unwrap();
        // a baseline belongs to its inputs, even where a changed config
        // gives the same output
        differs |= k == "inputs" && a != b;
    }
    for k in PARAMS {
        if base.head.get(k) != new.head.get(k) {
            writeln!(out, "- **not comparable**: {k} is {:?} in the baseline, {:?} in the results", base.head.get(k), new.head.get(k)).unwrap();
            print!("{out}");
            return Ok(ExitCode::FAILURE);
        }
    }

    let ids: std::collections::BTreeSet<&String> = base.recs.keys().filter(|id| in_shard(id, sh)).chain(new.recs.keys()).collect();
    let (mut same, mut finer) = (0usize, 0usize);
    let (mut added, mut removed) = (vec![], vec![]);
    // (|team move|, id, text)
    let mut moved: Vec<(f64, String, String)> = vec![];
    let mut team_moves: Vec<f64> = vec![];
    for id in ids {
        match (base.recs.get(id), new.recs.get(id)) {
            (Some(a), Some(b)) if a.sha == b.sha && a == b => same += 1,
            (Some(a), Some(b)) => {
                let mut parts = vec![];
                let mut m = 0.0;
                if a.error != b.error {
                    parts.push(format!("error `{}` -> `{}`", a.error.as_deref().unwrap_or("none"), b.error.as_deref().unwrap_or("none")));
                    m = f64::INFINITY;
                }
                let ta = a.team.as_deref().and_then(num);
                let tb = b.team.as_deref().and_then(num);
                if let (Some(x), Some(y)) = (ta, tb) {
                    if x != y {
                        parts.push(format!("team {x:.2} -> {y:.2} ({})", pct(x, y)));
                        if x.abs() > 1e-9 {
                            let r = (y - x) / x * 100.0;
                            m = f64::max(m, r.abs());
                            team_moves.push(r);
                        }
                    }
                }
                for (c, vb) in &b.chars {
                    let va = a.chars.iter().find(|(k, _)| k == c).map(|(_, v)| v.as_str());
                    match (va.and_then(num), num(vb)) {
                        (Some(x), Some(y)) if x != y => {
                            // judged by compare: at least 10% of the team's DPS
                            let judged = tb.is_some_and(|t| t > 0.0 && y / t >= 0.1) || ta.is_some_and(|t| t > 0.0 && x / t >= 0.1);
                            parts.push(format!("{c}{} {x:.2} -> {y:.2} ({})", if judged { "" } else { " (minor)" }, pct(x, y)));
                        }
                        (None, _) => parts.push(format!("{c} new")),
                        _ => {}
                    }
                }
                let mut wk: Vec<&String> = a.waits.iter().chain(&b.waits).map(|(k, _)| k).collect();
                wk.sort();
                wk.dedup();
                for k in wk {
                    let g = |r: &Rec| r.waits.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone()).unwrap_or_else(|| "0".into());
                    let (x, y) = (g(a), g(b));
                    if x != y {
                        parts.push(format!("wait {k} {x} -> {y}"));
                    }
                }
                if a.frames != b.frames {
                    parts.push(format!("frames {:?} -> {:?}", a.frames, b.frames));
                }
                if parts.is_empty() {
                    finer += 1;
                    parts.push("only the hash (finer than the printed digits, or abilities' hits and damage)".into());
                }
                moved.push((m, id.clone(), parts.join("; ")));
            }
            (None, Some(_)) => added.push(id.clone()),
            (Some(_), None) => removed.push(id.clone()),
            (None, None) => unreachable!(),
        }
    }
    differs |= !moved.is_empty() || !added.is_empty() || !removed.is_empty();
    moved.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

    writeln!(out, "- builds: {} unchanged, {} changed ({} only in the hash), {} new, {} missing", same, moved.len(), finer, added.len(), removed.len()).unwrap();
    if !team_moves.is_empty() {
        let n = team_moves.len() as f64;
        let mean = team_moves.iter().sum::<f64>() / n;
        let max = team_moves.iter().fold(0.0f64, |m, x| if x.abs() > m.abs() { *x } else { m });
        let over1 = team_moves.iter().filter(|x| x.abs() > 1.0).count();
        writeln!(out, "- team DPS moved in {} builds: mean {mean:+.4}%, largest {max:+.3}%, {over1} beyond ±1%", team_moves.len()).unwrap();
    }
    if !moved.is_empty() {
        writeln!(out, "\n### Changed, largest team move first\n").unwrap();
        for (_, id, t) in &moved {
            writeln!(out, "- `{id}`: {t}").unwrap();
        }
    }
    for (what, list) in [("New (not in the baseline)", &added), ("Missing (in the baseline, not in the results)", &removed)] {
        if !list.is_empty() {
            writeln!(out, "\n### {what}\n").unwrap();
            for id in list {
                writeln!(out, "- `{id}`").unwrap();
            }
        }
    }
    writeln!(out, "\n{}", if differs { "**The results differ from the baseline.**" } else { "The results equal the baseline." }).unwrap();
    print!("{out}");
    Ok(if differs { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = r#"{
  "iterations": 20,
  "warmup": 150,
  "seconds": 0.0836,
  "workers": 1,
  "team": {"mean": 72239.7886, "stderr": 442.0480},
  "frames": 27000,
  "chars": {
    "hutao": {"mean": 45275.2961, "stderr": 220.7362},
    "yelan": {"mean": 15622.0462, "stderr": 191.4132}
  },
  "waits": {
    "hutao": {},
    "yelan": {"swap_cd": 1.0333333333333334, "energy": 2}
  },
  "abils": {
    "hutao/Blood Blossom": [52.8, 834134.4060202203]
  }
}
"#;

    #[test]
    fn reads_output_and_round_trips() {
        let mut r = read_output(OUT);
        r.sha = "ab".into();
        assert_eq!(r.team.as_deref(), Some("72239.7886"));
        assert_eq!(r.frames.as_deref(), Some("27000"));
        assert_eq!(r.chars, vec![("hutao".into(), "45275.2961".into()), ("yelan".into(), "15622.0462".into())]);
        assert_eq!(r.waits, vec![("yelan/swap_cd".into(), "1.0333333333333334".into()), ("yelan/energy".into(), "2".into())]);
        let line = r.line("t-library");
        let (id, back) = Rec::parse(&line).unwrap();
        assert_eq!(id, "t-library");
        assert!(back == r);
        let e = Rec { sha: "cd".into(), team: None, chars: vec![], waits: vec![], frames: None, error: Some("1 iteration 0: unsupported: a b=c".into()) };
        let (_, back) = Rec::parse(&e.line("x")).unwrap();
        assert!(back == e);
    }

    #[test]
    fn shards_partition() {
        let ids = ["a", "b", "c", "amber-furina-bennett-rosaria-library"];
        for id in ids {
            assert_eq!((0..4).filter(|k| in_shard(id, Some((*k, 4)))).count(), 1);
        }
    }
}
