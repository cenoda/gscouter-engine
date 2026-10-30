//! The configs the engine is checked on. Base stats are what gcsim set up
//! for each character (reference/dump); rotations are compiled by hand
//! from the gcsim configs in configs/.

use crate::sim::*;
use crate::types::*;

fn stats(pairs: &[(&str, f64)]) -> Stats {
    let mut s = [0.0; N_STATS];
    for (k, v) in pairs {
        s[st::by_name(k).unwrap_or_else(|| panic!("stat {k}"))] = *v;
    }
    s
}

fn hutao() -> CharSetup {
    CharSetup {
        kind: CharKind::Hutao,
        base: stats(&[
            ("atk", 367.0),
            ("atk%", 0.157),
            ("base_atk", 714.5089773),
            ("base_def", 876.1519817999999),
            ("base_hp", 15552.3066196),
            ("cd", 2.120536),
            ("cr", 0.816),
            ("def", 21.0),
            ("em", 268.0),
            ("er", 1.12),
            ("hp", 5587.0),
            ("hp%", 0.19899999999999998),
            ("pyro%", 0.466),
        ]),
        level: 90,
        cons: 1,
        ascension: 6,
        talents: [10, 10, 10],
        weapon: Weapon::Homa,
        refine: 1,
        set: Some(Set::Shimenawa4),
    }
}

/// configs/hutao-solo-looped.txt
fn hutao_solo_looped() -> Config {
    use Action::*;
    let h = 0;
    let mut p = vec![Ins::Act(h, Skill, 0), Ins::Act(h, Attack, 0), Ins::Act(h, Charge, 0), Ins::Act(h, Dash, 0)];
    for _ in 0..7 {
        p.extend([Ins::Act(h, Attack, 0), Ins::Act(h, Attack, 0), Ins::Act(h, Charge, 0), Ins::Act(h, Dash, 0)]);
    }
    let n = p.len();
    p.extend([
        Ins::JmpUnless(Cond::BurstReady(h), false, n + 2),
        Ins::Act(h, Burst, 0),
        Ins::JmpUnless(Cond::SkillReady(h), true, n + 5),
        Ins::Act(h, Attack, 0),
        Ins::Jmp(n + 2),
        Ins::Jmp(0),
    ]);
    Config {
        chars: vec![hutao()],
        active: 0,
        duration: 450.0,
        enemy_level: 100,
        resist: 0.1,
        energy_interval: (480, 720),
        swap_delay: 12,
        program: p,
    }
}

pub fn get(name: &str) -> Option<Config> {
    match name {
        "hutao-solo-looped" => Some(hutao_solo_looped()),
        _ => None,
    }
}
