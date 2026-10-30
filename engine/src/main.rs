//! Runs a gcsim config and prints the numbers the reference harness prints
//! for gcsim, so the two can be compared.
//!
//! Usage: gse <config> [iterations] [warmup] [threads]
//!
//! iterations defaults to the config's `options iteration`.

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

use std::time::Instant;

type Row = (f64, [f64; 4], [f64; 4]);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: gse <config> [iterations] [warmup] [threads]");
        std::process::exit(2);
    };
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
    let parsed = gcsl::parse(&src).unwrap_or_else(|e| fail(&e));
    let cfg = profile::config(&parsed).unwrap_or_else(|e| fail(&e));
    let iters: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(parsed.iterations as usize);
    let warmup: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let threads: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    let names: Vec<&str> = parsed.chars.iter().map(|c| gcsl::char_name(c.key)).collect();
    let n_chars = names.len();
    drop(cfg);
    drop(parsed);

    let start = Instant::now();
    let mut results: Vec<Row> = vec![(0.0, [0.0; 4], [0.0; 4]); iters];
    let abils = std::sync::Mutex::new([[(0.0f64, 0.0f64); 32]; 4]);
    let chunk = iters.div_ceil(threads).max(1);
    // each thread parses the config itself: the program's AST is not Sync
    let run_part = |t: usize, part: &mut [Row]| -> Result<(), String> {
        let parsed = gcsl::parse(&src)?;
        let cfg = profile::config(&parsed)?;
        for (j, r) in part.iter_mut().enumerate() {
            let i = t * chunk + j;
            let mut sim = sim::Sim::new(&cfg, 1 + i as u64, warmup);
            eval::run(&mut sim, &parsed.program).map_err(|e| format!("iteration {i}: {e}"))?;
            let window = (sim.f - sim.window_start) as f64 / 60.0;
            let mut chars = [0.0; 4];
            for c in 0..n_chars {
                chars[c] = sim.window_char[c] / window;
            }
            *r = (chars.iter().sum(), chars, sim.waits);
            let mut a = abils.lock().unwrap();
            for c in 0..4 {
                for k in 0..32 {
                    a[c][k].0 += sim.abils[c][k].0;
                    a[c][k].1 += sim.abils[c][k].1;
                }
            }
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

    let stat = |xs: &[f64]| {
        let n = xs.len() as f64;
        let m = xs.iter().sum::<f64>() / n;
        let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0).max(1.0);
        (m, (v / n).sqrt())
    };
    let team: Vec<f64> = results.iter().map(|r| r.0).collect();
    let (m, se) = stat(&team);
    println!("{{");
    println!("  \"iterations\": {iters}, \"warmup\": {warmup}, \"seconds\": {secs:.4}, \"threads\": {threads},");
    println!("  \"team\": {{\"mean\": {m:.2}, \"stderr\": {se:.2}}},");
    println!("  \"chars\": {{");
    for c in 0..n_chars {
        let xs: Vec<f64> = results.iter().map(|r| r.1[c]).collect();
        let (m, se) = stat(&xs);
        let w: f64 = results.iter().map(|r| r.2[c]).sum::<f64>() / iters as f64;
        let comma = if c + 1 < n_chars { "," } else { "" };
        println!("    \"{}\": {{\"mean\": {m:.2}, \"stderr\": {se:.2}, \"waits\": {w:.3}}}{comma}", names[c]);
    }
    println!("  }},");
    println!("  \"abils\": {{");
    let a = abils.lock().unwrap();
    let mut rows = Vec::new();
    for c in 0..n_chars {
        for k in 0..32 {
            if a[c][k].0 > 0.0 {
                rows.push(format!("    \"{}/{k}\": [{:.3}, {:.1}]", names[c], a[c][k].0 / iters as f64, a[c][k].1 / iters as f64));
            }
        }
    }
    println!("{}", rows.join(",\n"));
    println!("  }}");
    println!("}}");
}

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}
