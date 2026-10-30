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

fn xingqiu() -> CharSetup {
    CharSetup {
        kind: CharKind::Xingqiu,
        base: stats(&[
            ("atk", 327.0),
            ("atk%", 0.8809999999999999),
            ("base_atk", 656.1404849),
            ("base_def", 757.5988927500001),
            ("base_hp", 10222.420242440001),
            ("cd", 1.355),
            ("cr", 0.524),
            ("def", 63.0),
            ("def%", 0.23399999999999999),
            ("em", 147.0),
            ("er", 1.832531802),
            ("hp", 5288.0),
            ("hydro%", 0.466),
        ]),
        level: 90,
        cons: 6,
        ascension: 6,
        talents: [9, 9, 10],
        weapon: Weapon::SacrificialSword,
        refine: 4,
        set: Some(Set::Emblem4),
    }
}

fn zhongli() -> CharSetup {
    CharSetup {
        kind: CharKind::Zhongli,
        base: stats(&[
            ("atk", 344.0),
            ("atk%", 0.6759999999999999),
            ("base_atk", 859.2118862),
            ("base_def", 737.8121952),
            ("base_hp", 14695.093175999998),
            ("cd", 1.6595360000000001),
            ("cr", 0.559),
            ("def", 42.0),
            ("def%", 0.057999999999999996),
            ("em", 40.0),
            ("er", 1.324),
            ("geo%", 0.754),
            ("hp", 5318.0),
            ("hp%", 0.396),
        ]),
        level: 90,
        cons: 0,
        ascension: 6,
        talents: [9, 10, 9],
        weapon: Weapon::Homa,
        refine: 1,
        set: Some(Set::Tenacity4),
    }
}

fn yelan() -> CharSetup {
    CharSetup {
        kind: CharKind::Yelan,
        base: stats(&[
            ("atk", 329.0),
            ("base_atk", 785.7917076),
            ("base_def", 547.97926581),
            ("base_hp", 14450.174906400001),
            ("cd", 2.058048),
            ("cr", 0.6739999999999999),
            ("def", 19.0),
            ("def%", 0.182),
            ("em", 100.0),
            ("er", 1.725),
            ("hp", 4780.0),
            ("hp%", 0.658),
            ("hydro%", 0.466),
        ]),
        level: 90,
        cons: 0,
        ascension: 6,
        talents: [1, 9, 10],
        weapon: Weapon::AquaSimulacra,
        refine: 1,
        // Emblem 4pc; the Noblesse 1pc does nothing
        set: Some(Set::Emblem4),
    }
}

/// A rotation being compiled: actions, and jumps to labels fixed up at the end.
struct Asm {
    ins: Vec<Ins>,
}

impl Asm {
    fn act(&mut self, c: u8, a: Action) {
        self.ins.push(Ins::Act(c, a, 0));
    }
    fn here(&self) -> usize {
        self.ins.len()
    }
    /// `if cond { body }`
    fn if_(&mut self, cond: Cond, body: impl FnOnce(&mut Asm)) {
        let at = self.here();
        self.ins.push(Ins::Jmp(0));
        body(self);
        let end = self.here();
        self.ins[at] = Ins::JmpUnless(cond, false, end);
    }
    /// `while !cond { body }`
    fn until(&mut self, cond: Cond, body: impl FnOnce(&mut Asm)) {
        let top = self.here();
        self.ins.push(Ins::Jmp(0));
        body(self);
        self.ins.push(Ins::Jmp(top));
        let end = self.here();
        self.ins[top] = Ins::JmpUnless(cond, true, end);
    }
}

/// configs/team-looped.txt: Gscouter's looped score config for
/// teams/hutao-xingqiu-zhongli-yelan.
fn team_looped() -> Config {
    use Action::*;
    let (ht, xq, zl, ye) = (0u8, 1u8, 2u8, 3u8);
    let mut a = Asm { ins: Vec::new() };
    a.ins.push(Ins::Act(zl, Skill, 1));
    a.act(ye, Skill);
    a.if_(Cond::BurstReady(ye), |a| {
        a.act(ye, Burst);
        a.act(ye, Attack);
    });
    a.if_(Cond::BurstReady(xq), |a| a.act(xq, Burst));
    a.until(Cond::SkillReady(xq), |a| a.act(xq, Attack));
    a.act(xq, Skill);
    a.if_(Cond::SkillReady(xq), |a| a.act(xq, Skill));
    a.if_(Cond::BurstReady(xq), |a| a.act(xq, Burst));
    a.if_(Cond::BurstReady(ye), |a| {
        a.act(ye, Burst);
        a.act(ye, Attack);
    });
    a.act(ht, Skill);
    a.act(ht, Attack);
    a.act(ht, Charge);
    a.act(ht, Dash);
    for _ in 0..9 {
        a.act(ht, Attack);
        a.act(ht, Attack);
        a.act(ht, Charge);
        a.act(ht, Dash);
    }
    a.if_(Cond::BurstReady(ht), |a| a.act(ht, Burst));
    a.ins.push(Ins::Jmp(0));
    Config {
        chars: vec![hutao(), xingqiu(), zhongli(), yelan()],
        active: zl as usize,
        duration: 450.0,
        enemy_level: 100,
        resist: 0.1,
        energy_interval: (480, 720),
        swap_delay: 12,
        program: a.ins,
    }
}

pub fn get(name: &str) -> Option<Config> {
    match name {
        "hutao-solo-looped" => Some(hutao_solo_looped()),
        "team-looped" => Some(team_looped()),
        _ => None,
    }
}
