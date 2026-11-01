//! The engine as a library: the `gse` command (`main.rs`) and the entry
//! points Gscouter calls (`api`) share it.

pub mod api;
mod chars;
pub mod check;
mod content;
mod data;
pub mod eval;
mod gcsimdata;
pub mod gcsl;
pub mod profile;
mod queue;
mod rng;
mod sets;
pub mod sim;
pub mod types;
mod weapons;
