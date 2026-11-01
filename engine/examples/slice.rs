//! Prints what `api::run_iterations` gives, as Gscouter's JSON:
//!
//!   cargo run --release --example slice -- <config> <warmup> <seed> <first> <count>
//!
//! The WebAssembly module (`wasm/`) must print the same, byte for byte.

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 5 {
        eprintln!("usage: slice <config> <warmup> <seed> <first> <count>");
        std::process::exit(2);
    }
    let src = std::fs::read_to_string(&a[0]).unwrap_or_else(|e| panic!("{}: {e}", a[0]));
    let n = |i: usize| a[i].parse::<f64>().unwrap_or_else(|_| panic!("{} is not a number", a[i]));
    match gse::api::run_iterations(&src, n(1), n(2) as i64, n(3) as usize, n(4) as usize) {
        Ok(its) => println!("{}", gse::api::to_json(&its)),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
