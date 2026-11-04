//! Cashflow Supervision, from gcsim's internal/weapons/catalyst/cashflow.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK: ModKey = ModKey::new("cashflow-supervision-atkp");
const BUFF: ModKey = ModKey::new("cashflow-buff");
const ICD: ModKey = ModKey::new("cashflow-icd");
const ATKSPD: ModKey = ModKey::new("cashflow-atkspd");
const STELLAR: ModKey = ModKey::new("cashflow-stellar");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "cashflowsupervision";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// ATK +16 to 32%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.12 + 0.04 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Drain, o);
        s.subscribe(EventKind::Heal, o);
    }

    /// Per stack: normal attack DMG +16 to 32%, charged attack DMG +14 to
    /// 28%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let n = s.weapon_st::<State>(ci).stacks as f64;
        match ev.info.tag {
            AttackTag::Normal => ev.stats[st::DMGP] += (0.12 + 0.04 * r) * n,
            AttackTag::Extra => ev.stats[st::DMGP] += (0.105 + 0.035 * r) * n,
            _ => {}
        }
    }

    /// Per stack: Stellar-Conduct DMG (direct) +14 to 28%.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag != AttackTag::DirectStellarConduct {
            return 0.0;
        }
        (0.105 + 0.035 * s.chars[ci].refine as f64) * s.weapon_st::<State>(ci).stacks as f64
    }

    /// At three stacks, ATK SPD +8 to 16% during normal attacks.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += 0.06 + 0.02 * s.chars[ci].refine as f64;
        true
    }

    /// The holder's HP drained or healed (by more than the overheal) on
    /// field, once every 0.3 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let changed = match *e {
            Event::Drain { on, amount, .. } => on == ci && amount > 0.0,
            Event::Heal { on, amount, overheal, .. } => on == ci && amount > 0.0 && (amount - overheal).abs() > 1e-9,
            _ => false,
        };
        if !changed || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        on_change_hp(s, ci);
    }
}

/// gcsim's onChangeHP: a stack (from none once the buff is gone, at most
/// three), the buffs for 4 s (hitlag), and ATK SPD at three stacks.
fn on_change_hp(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, BUFF) {
        s.weapon_st_mut::<State>(ci).stacks = 0;
    }
    let st = s.weapon_st_mut::<State>(ci);
    if st.stacks < 3 {
        st.stacks += 1;
    }
    let stacks = st.stacks;
    let o = Owner::Weapon(ci as u8);
    s.add_status(ci, ICD, 18, true);
    s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, o, st::NONE, false, true, -1), 4 * 60);
    s.add_mod(ci, Mod::owned(STELLAR, ModKind::ReactBonus, o, st::NONE, false, true, -1), 4 * 60);
    if stacks == 3 {
        s.add_mod(ci, Mod::owned(ATKSPD, ModKind::Stat, o, st::ATKSPD, false, true, -1), 4 * 60);
    }
}
