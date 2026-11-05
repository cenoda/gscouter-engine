//! Mechanic checks from evidence, not from agreement with gcsim (Phase 6
//! step 3a, `docs/mechanics-tests.md`).
//!
//! Each check runs a controlled fixture (`tests/mechanics/<character>/`):
//! one character, one enemy, no crits (`cr=-0.05`), no team buffs, one
//! iteration. It reads hits and damage by ability from `gse` and compares
//! them with a value computed by hand from the cited sources. A tolerance
//! comes from the display rounding of those sources, never from what gcsim
//! gives.
//!
//! A check whose evidence disagrees with the engine is `Known`: it asserts
//! the engine's current value and names the evidence that disagrees, so a
//! change either way has to update the record (`docs/mechanics-tests.md`).

use std::collections::BTreeMap;
use std::process::Command;

/// Hits and damage by ability (`"char/ability": [hits, damage]`) of one
/// iteration of a fixture.
fn abils(fixture: &str) -> BTreeMap<String, (f64, f64)> {
    let path = format!("{}/tests/mechanics/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let out = Command::new(env!("CARGO_BIN_EXE_gse")).args([&path, "1", "0", "1"]).output().expect("run gse");
    assert!(out.status.success(), "{fixture}: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    let body = &s[s.find("\"abils\"").expect("abils")..];
    let mut m = BTreeMap::new();
    for line in body.lines().skip(1) {
        let line = line.trim().trim_end_matches(',');
        let Some((k, v)) = line.split_once(": [") else { continue };
        let (h, d) = v.trim_end_matches(']').split_once(", ").unwrap();
        m.insert(k.trim_matches('"').to_string(), (h.parse().unwrap(), d.parse().unwrap()));
    }
    m
}

fn hits(m: &BTreeMap<String, (f64, f64)>, k: &str) -> f64 {
    m.get(k).map_or(0.0, |v| v.0)
}

/// Damage of one hit of an ability whose hits all deal the same.
fn per_hit(m: &BTreeMap<String, (f64, f64)>, k: &str) -> f64 {
    let (h, d) = m[k];
    d / h
}

/// A value read from a source shown to `step` (it lies within ±step/2),
/// with its relative half-width.
#[derive(Clone, Copy)]
struct Shown {
    v: f64,
    step: f64,
}

impl Shown {
    fn rel(self) -> f64 {
        self.step / 2.0 / self.v
    }
}

/// Xiangling at Lv.90/90 with Beginner's Protector at Lv.70/70 against a
/// Lv.100 enemy with 10% resistance, no crits.
mod xiangling {
    use super::*;

    /// Base ATK at Lv.90 (ascension 6): 225, HoYoWiki entry 50 (official,
    /// `docs/pilot-xiangling/official.json`, `xiangling.base_atk@lv90.a6`).
    const BASE_ATK: Shown = Shown { v: 225.0, step: 1.0 };
    /// Beginner's Protector at Lv.70/70: 185. Community stat tables (e.g.
    /// zilliongamer.com's level table, read 2026-11-05); no official page
    /// was found. It has no passive.
    const WEAPON_ATK: Shown = Shown { v: 185.0, step: 1.0 };
    /// DEF multiplier (Lc+100)/((Lc+100)+(Le+100)) and RES multiplier
    /// 1−RES for 0 ≤ RES < 75%: HoYoLAB's Damage Formula page (entry 5654),
    /// a community source (owner, 2026-11-05). Exact for these levels.
    const DEF_MULT: f64 = 190.0 / (190.0 + 200.0);
    const RES_MULT: f64 = 0.9;

    /// Expected damage of a hit of `mult` of ATK, and its relative
    /// tolerance: the sum of the sources' rounding, first order.
    fn expect(mult: Shown) -> (f64, f64) {
        let atk = BASE_ATK.v + WEAPON_ATK.v;
        let atk_rel = (BASE_ATK.step + WEAPON_ATK.step) / 2.0 / atk;
        (mult.v * atk * DEF_MULT * RES_MULT, atk_rel + mult.rel())
    }

    fn check_hit(m: &BTreeMap<String, (f64, f64)>, k: &str, mult: Shown) {
        let (want, tol) = expect(mult);
        let got = per_hit(m, k);
        let rel = got / want - 1.0;
        assert!(rel.abs() <= tol, "{k}: engine {got:.3}, evidence {want:.3} (±{:.3}%), off by {:.3}%", tol * 100.0, rel * 100.0);
    }

    /// Talent multipliers at Lv.10, HoYoWiki entry 50 (official), shown to
    /// three significant figures below 100% and to the integer above.
    const fn pct(v: f64, step: f64) -> Shown {
        Shown { v: v / 100.0, step: step / 100.0 }
    }

    #[test]
    fn normal_attack_hits_and_damage() {
        let m = abils("xiangling/normals.txt");
        // Hit counts: the talent table's "3-Hit DMG 51.5%×2", "4-Hit DMG
        // 27.9%×4" (official).
        for (k, n) in [("Normal 0", 1.0), ("Normal 1", 1.0), ("Normal 2", 2.0), ("Normal 3", 4.0), ("Normal 4", 1.0)] {
            assert_eq!(hits(&m, &format!("xiangling/{k}")), n, "{k} hits");
        }
        check_hit(&m, "xiangling/Normal 0", pct(83.1, 0.1));
        check_hit(&m, "xiangling/Normal 1", pct(83.3, 0.1));
        check_hit(&m, "xiangling/Normal 2", pct(51.5, 0.1));
        check_hit(&m, "xiangling/Normal 3", pct(27.9, 0.1));
        check_hit(&m, "xiangling/Normal 4", pct(140.0, 1.0));
    }

    #[test]
    fn charged_attack() {
        let m = abils("xiangling/charged.txt");
        assert_eq!(hits(&m, "xiangling/Charge"), 1.0);
        check_hit(&m, "xiangling/Charge", pct(241.0, 1.0));
    }

    #[test]
    fn guoba_breaths() {
        let m = abils("xiangling/guoba.txt");
        // "breathing fire ... every 1.5 seconds 4 times" (official text)
        assert_eq!(hits(&m, "xiangling/Guoba"), 4.0);
        check_hit(&m, "xiangling/Guoba", pct(200.0, 1.0));
    }

    #[test]
    fn pyronado_c0() {
        let m = abils("xiangling/pyronado-c0.txt");
        // three swings (official table: 1-, 2-, 3-Hit Swing DMG)
        check_hit(&m, "xiangling/Pyronado Hit 1", pct(130.0, 1.0));
        check_hit(&m, "xiangling/Pyronado Hit 2", pct(158.0, 1.0));
        check_hit(&m, "xiangling/Pyronado Hit 3", pct(197.0, 1.0));
        check_hit(&m, "xiangling/Pyronado", pct(202.0, 1.0));
        // Known: 9 spin hits in 10 s. KQM's guide (stated) gives 8 standing
        // still, and the "Pyronado and Glacial Waltz frame counts" sheet
        // (measured) 8 cycles; `docs/pilot-xiangling/hidden.md` §2, §3 B.
        // A step 6 candidate, not changed here.
        assert_eq!(hits(&m, "xiangling/Pyronado"), 9.0, "Known difference: evidence says 8; update docs/mechanics-tests.md");
    }

    #[test]
    fn pyronado_c4_hits() {
        let m = abils("xiangling/pyronado-c4.txt");
        // C4: duration +40% (official); 12 spin hits standing still (KQM's
        // guide, stated). Damage is not checked: C3 raises the burst to
        // Lv.13.
        assert_eq!(hits(&m, "xiangling/Pyronado"), 12.0);
    }

    #[test]
    fn energy() {
        // Starts full at 80, the burst's cost (official).
        assert_eq!(hits(&abils("xiangling/energy-start.txt"), "xiangling/Normal 0"), 1.0, "starts with 80 energy");
        // The burst takes all of it.
        assert_eq!(hits(&abils("xiangling/energy-burst.txt"), "xiangling/Normal 0"), 1.0, "burst leaves 0 energy");
        // Guoba: one particle per breath that hits (KQM's guide, stated) ×
        // 4 breaths; a same-element particle gives 3 energy on field at
        // 100% ER (KQM TCL's energy page, community): 12.
        assert_eq!(hits(&abils("xiangling/energy-guoba.txt"), "xiangling/Normal 0"), 1.0, "Guoba gives 12 energy");
    }
}
