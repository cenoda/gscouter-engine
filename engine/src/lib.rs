//! The engine as a library: the `gse` command (`main.rs`) and the entry
//! points Gscouter calls (`api`) share it. `main.rs` declares the same
//! modules (see there why); a test checks that the two lists agree.

pub mod api;
mod chars;
pub mod check;
mod content;
mod data;
pub mod eval;
mod gamedata;
pub mod gcsl;
mod nightsoul;
pub mod profile;
mod queue;
mod rng;
mod sets;
pub mod sim;
mod stellar;
pub mod types;
mod weapons;

#[cfg(test)]
mod tests {
    /// The modules a file declares, without `pub`.
    fn mods(src: &str) -> Vec<String> {
        let mut v: Vec<String> = src
            .lines()
            .filter_map(|l| l.trim().trim_start_matches("pub ").strip_prefix("mod ")?.strip_suffix(';').map(String::from))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn main_and_lib_declare_the_same_modules() {
        let mut lib = mods(include_str!("lib.rs"));
        lib.retain(|m| m != "api");
        assert_eq!(mods(include_str!("main.rs")), lib, "add a module to both main.rs and lib.rs");
    }
}
