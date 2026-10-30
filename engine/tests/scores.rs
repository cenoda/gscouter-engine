//! The engine's scores must not move when its data or structure changes:
//! the same seeds give the same fights, to the last digit.

use std::process::Command;

fn team_mean(config: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_gse"))
        .args([&format!("{}/../configs/{config}", env!("CARGO_MANIFEST_DIR")), "1000", "150", "4"])
        .output()
        .expect("run gse");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    let line = s.lines().find(|l| l.trim_start().starts_with("\"team\"")).expect("team line");
    let i = line.find("\"mean\": ").unwrap() + 8;
    line[i..].split(',').next().unwrap().to_string()
}

#[test]
fn team_looped() {
    assert_eq!(team_mean("team-looped.txt"), "72266.7294");
}

#[test]
fn hutao_solo_looped() {
    assert_eq!(team_mean("hutao-solo-looped.txt"), "24607.0319");
}
