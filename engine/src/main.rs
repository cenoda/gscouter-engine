//! Runs a gcsim config and prints what the reference harness prints for
//! gcsim, in the same JSON, so the two can be compared.
//!
//! Usage:
//!
//!   gse <config> [iterations] [warmup] [threads] [--seed N]
//!   gse --check <config>
//!
//! iterations defaults to the config's `options iteration`; iteration i
//! uses seed N + i (N defaults to 1). --check prints what the config needs
//! that the engine does not implement, as {"unsupported": [...]}, without
//! running it.

mod check;
mod data;
mod eval;
mod gcsl;
mod hutao;
mod profile;
mod queue;
mod rng;
mod sim;
mod types;
mod xingqiu;
mod yelan;
mod zhongli;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use types::{FAILURE_NAMES, N_FAILURES};

/// One iteration: team and character DPS over the window, and frames waited
/// by character and reason.
struct Row {
    team: f64,
    chars: [f64; 4],
    waits: [[u32; N_FAILURES]; 4],
    frames: i32,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed: u64 = 1;
    if let Some(i) = args.iter().position(|a| a == "--seed") {
        seed = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or_else(|| fail("--seed needs a number"));
        args.drain(i..i + 2);
    }
    let check = match args.iter().position(|a| a == "--check") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let Some(path) = args.first() else {
        eprintln!("usage: gse <config> [iterations] [warmup] [threads] [--seed N]\n       gse --check <config>");
        std::process::exit(2);
    };
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
    let parsed = match gcsl::parse(&src) {
        Ok(p) => p,
        Err(e) if check && e.starts_with("unsupported: ") => {
            println!("{{\"unsupported\": [{}]}}", json_str(&e));
            return;
        }
        Err(e) => fail(&e),
    };
    if check {
        let list: Vec<String> = check::unsupported(&parsed).iter().map(|s| json_str(s)).collect();
        println!("{{\"unsupported\": [{}]}}", list.join(", "));
        return;
    }
    let cfg = profile::config(&parsed).unwrap_or_else(|e| fail(&e.join("; ")));
    let iters: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(parsed.iterations as usize);
    let warmup: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let names: Vec<&str> = parsed.chars.iter().map(|c| gcsl::char_name(c.key)).collect();
    let n_chars = names.len();
    drop(cfg);
    drop(parsed);

    let start = Instant::now();
    let mut results: Vec<Row> = (0..iters).map(|_| Row { team: 0.0, chars: [0.0; 4], waits: [[0; N_FAILURES]; 4], frames: 0 }).collect();
    let abils = std::sync::Mutex::new(BTreeMap::<String, (f64, f64)>::new());
    let chunk = iters.div_ceil(threads).max(1);
    // each thread parses the config itself: the program's AST is not Sync
    let run_part = |t: usize, part: &mut [Row]| -> Result<(), String> {
        let parsed = gcsl::parse(&src)?;
        let cfg = profile::config(&parsed).map_err(|e| e.join("; "))?;
        let mut local: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        for (j, r) in part.iter_mut().enumerate() {
            let i = t * chunk + j;
            let mut sim = sim::Sim::new(&cfg, seed + i as u64, warmup);
            eval::run(&mut sim, &parsed.program).map_err(|e| format!("iteration {i}: {e}"))?;
            let window = (sim.f - sim.window_start) as f64 / 60.0;
            for c in 0..n_chars {
                r.chars[c] = sim.window_char[c] / window;
            }
            r.team = r.chars.iter().sum();
            r.waits = sim.waits;
            r.frames = sim.f;
            for c in 0..n_chars {
                for (name, hits, dmg) in &sim.abils[c] {
                    let e = local.entry(format!("{}/{name}", names[c])).or_default();
                    e.0 += *hits as f64;
                    e.1 += dmg;
                }
            }
        }
        let mut a = abils.lock().unwrap();
        for (k, v) in local {
            let e = a.entry(k).or_default();
            e.0 += v.0;
            e.1 += v.1;
        }
        Ok(())
    };
    let res = if threads == 1 {
        // also the WebAssembly build, which has no threads
        run_part(0, &mut results)
    } else {
        std::thread::scope(|s| {
            let handles: Vec<_> = results
                .chunks_mut(chunk)
                .enumerate()
                .map(|(t, part)| {
                    let run_part = &run_part;
                    s.spawn(move || run_part(t, part))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect::<Result<(), String>>()
        })
    };
    if let Err(e) = res {
        fail(&e);
    }
    let secs = start.elapsed().as_secs_f64();

    let n = iters as f64;
    let stat = |xs: &[f64]| {
        let m = xs.iter().sum::<f64>() / n;
        let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0).max(1.0);
        format!("{{\"mean\": {m:.4}, \"stderr\": {:.4}}}", (v / n).sqrt())
    };
    let mut o = String::new();
    let team: Vec<f64> = results.iter().map(|r| r.team).collect();
    let frames = results.iter().map(|r| r.frames as f64).sum::<f64>() / n;
    let _ = writeln!(o, "{{\n  \"iterations\": {iters},\n  \"warmup\": {warmup},\n  \"seconds\": {secs:.4},\n  \"workers\": {threads},");
    let _ = writeln!(o, "  \"team\": {},\n  \"frames\": {frames},", stat(&team));
    let chars: Vec<String> = (0..n_chars)
        .map(|c| {
            let xs: Vec<f64> = results.iter().map(|r| r.chars[c]).collect();
            format!("    {}: {}", json_str(names[c]), stat(&xs))
        })
        .collect();
    let _ = writeln!(o, "  \"chars\": {{\n{}\n  }},", chars.join(",\n"));
    let waits: Vec<String> = (0..n_chars)
        .map(|c| {
            let reasons: Vec<String> = (0..N_FAILURES)
                .filter_map(|k| {
                    let frames: u64 = results.iter().map(|r| r.waits[c][k] as u64).sum();
                    (frames > 0).then(|| format!("{}: {}", json_str(FAILURE_NAMES[k]), frames as f64 / 60.0 / n))
                })
                .collect();
            format!("    {}: {{{}}}", json_str(names[c]), reasons.join(", "))
        })
        .collect();
    let _ = writeln!(o, "  \"waits\": {{\n{}\n  }},", waits.join(",\n"));
    let a = abils.lock().unwrap();
    let rows: Vec<String> = a.iter().map(|(k, v)| format!("    {}: [{}, {}]", json_str(k), v.0 / n, v.1 / n)).collect();
    let _ = writeln!(o, "  \"abils\": {{\n{}\n  }}\n}}", rows.join(",\n"));
    print!("{o}");
}

fn json_str(s: &str) -> String {
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

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}
