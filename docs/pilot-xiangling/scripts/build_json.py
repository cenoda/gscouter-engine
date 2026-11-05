"""Assemble docs/pilot-xiangling/official.json from the pilot's observations.

usage: python3 -I build_json.py <talents.json> <data.rs> <out.json>
"""
import json, re, sys

talents = json.load(open(sys.argv[1]))
src = open(sys.argv[2]).read()
OBS = '2026-11-05'
PAGE = 'https://wiki.hoyolab.com/pc/genshin/entry/50?lang=en-us'
FORMULA = 'https://wiki.hoyolab.com/pc/genshin/entry/5654?lang=en-us'
ENKA = 'https://enka.network/api/uid/618285856'
GV = 'live on observation date (page does not state it; page revision 2025-12-08T07:14:18Z)'
PROV = 'legacy_gcsim'

def rec(key, unit, official, legacy, legacy_src, agree, note='', evidence='official', url=PAGE, diff=None):
    return {'key': key, 'unit': unit, 'game_version': GV, 'source_url': url, 'observed': OBS,
            'evidence': evidence, 'official': official, 'legacy': legacy, 'legacy_source': legacy_src,
            'legacy_provenance': PROV, 'agree': agree, 'difference': diff, 'note': note}

out = []
XL = 'engine/src/gcsimdata/chars/xiangling.rs'
for r in talents['talent_records']:
    out.append(rec(r['key'] + '@lv%d' % r['level'], 'fraction of ATK (page shows %)', r['official'], r['legacy'], XL,
                   r['agree'], 'compared at display precision: legacy rounded half-up to the shown decimals = ' + r['legacy_rounded'],
                   diff='%.4f percentage points (display rounding)' % r['diff_pct_points']))

CHR = 'engine/src/chars/xiangling.rs'
scal = [
    ('xiangling.charge.stamina', 'stamina', '25.0 (Lv1-11)', 25.0, 'engine/src/sim.rs abil_stam_cost (spear default)', ''),
    ('xiangling.skill.cd', 's', '12.0s (Lv1-13)', '12 * 60 frames', CHR + ' skill()', ''),
    ('xiangling.burst.duration', 's', '10.0s (Lv1-13)', '10 * 60 frames', CHR + ' TASK_SPIN_START', ''),
    ('xiangling.burst.cd', 's', '20.0s (Lv1-13)', '20 * 60 frames', CHR + ' burst()', ''),
    ('xiangling.burst.energy', 'energy', '80 (Lv1-13)', 80.0, 'engine/src/data.rs CharData.energy_max', 'also 80 in Gscouter internal/enka/data/character.dm.json burst_energy_cost'),
]
for k, u, o, l, s_, n in scal:
    out.append(rec(k, u, o, l, s_, True, n, diff='none'))

desc = [
    ('xiangling.a1.guoba_range', 'fraction', '+20% flame range of Guoba', 'range 5.0 * 1.2', CHR + ' TASK_GUOBA', 'agree', ''),
    ('xiangling.a4.atk', 'fraction ATK%', '+10% ATK', 0.1, CHR + ' TASK_A4', 'agree', 'pickup is modelled by the action parameter a4_delay (the page states a pickup, not when)'),
    ('xiangling.a4.duration', 's', '10s', '10 * 60 frames', CHR + ' TASK_A4', 'agree', ''),
    ('xiangling.a4.trigger', 'condition', "when Guoba Attack's effect ends; picked up", 'chili at Guoba expiry + a4_delay, given to the active character', CHR, 'agree', 'who picks it up and when is not on the page; legacy assumes the active character'),
    ('xiangling.c1.pyro_res', 'fraction', '-15% Pyro RES', -0.15, CHR + ' attack_cb CB_C1', 'agree', ''),
    ('xiangling.c1.duration', 's', '6s', '6 * 60 frames', CHR + ' attack_cb CB_C1', 'agree', ''),
    ('xiangling.c1.trigger', 'condition', "opponents hit by Guoba's attacks", 'Guoba breath callback', CHR, 'agree', ''),
    ('xiangling.c2.mult', 'fraction of ATK', '75% ATK, AoE Pyro', 0.75, CHR + ' TASK_C2_EXPLODE', 'agree', ''),
    ('xiangling.c2.delay', 's', 'Implode 2s, explodes when it ends', '120 frames', CHR + ' attack_cb CB_C2', 'agree', ''),
    ('xiangling.c2.trigger', 'condition', 'last attack in a Normal Attack sequence', 'normal_counter == 4 (5th hit)', CHR + ' attack()', 'agree', ''),
    ('xiangling.c3.burst_level', 'talent levels', '+3 Pyronado, max 15', 'cons [-1, 5, 3]: burst +3 at C3', 'engine/src/data.rs CharData.cons', 'agree', ''),
    ('xiangling.c4.duration', 'fraction', 'Pyronado duration +40%', '14 * 60 vs 10 * 60 frames', CHR + ' TASK_SPIN_START', 'agree', '10 s * 1.4 = 14 s'),
    ('xiangling.c5.skill_level', 'talent levels', '+3 Guoba Attack, max 15', 'cons [-1, 5, 3]: skill +3 at C5', 'engine/src/data.rs CharData.cons', 'agree', ''),
    ('xiangling.c6.pyro_dmg', 'fraction', '+15% Pyro DMG Bonus, all party members', 0.15, CHR + ' TASK_SPIN_START', 'agree', ''),
    ('xiangling.c6.duration', 'condition', 'for the duration of Pyronado', 'max (10 s, or 14 s at C4)', CHR + ' TASK_SPIN_START', 'agree', ''),
]
for k, u, o, l, s_, a, n in desc:
    out.append(rec(k, u, o, l, s_, a == 'agree', n, diff='none'))

# base stats
curves = {int(m.group(1)): [float(x) for x in m.group(2).split(',')] for m in re.finditer(r'\((\d+), &\[([0-9., e-]+)\]\)', src[src.find('pub static CURVES'):])}
base = [911.791, 18.87648, 56.0805]; cv = [22, 32, 22]
promo = [{}, {23: 681.15454, 25: 41.895, 24: 14.102028}, {23: 1165.1328, 25: 71.6625, 24: 24.12189, 8: 24.0},
         {23: 1810.4371, 25: 111.3525, 24: 37.481705, 8: 48.0}, {23: 2294.4153, 25: 141.12, 24: 47.501568, 8: 48.0},
         {23: 2778.3936, 25: 170.8875, 24: 57.52143, 8: 72.0}, {23: 3262.3718, 25: 200.655, 24: 67.54129, 8: 96.0}]
official = {  # level, ascension phase: HP, ATK, DEF, EM as the Ascend tab shows them
    (1, 0): (912, 19, 56, 0), (20, 0): (2342, 48, 144, 0), (20, 1): (3024, 63, 186, 0),
    (40, 1): (4529, 94, 279, 0), (40, 2): (5013, 104, 308, 24), (50, 2): (5766, 119, 355, 24),
    (50, 3): (6411, 133, 394, 48), (60, 3): (7164, 148, 441, 48), (60, 4): (7648, 158, 470, 48),
    (70, 4): (8401, 174, 517, 48), (70, 5): (8885, 184, 546, 72), (80, 5): (9638, 200, 593, 72),
    (80, 6): (10122, 210, 623, 96), (90, 6): (10875, 225, 669, 96)}
for (lvl, asc), vals in official.items():
    p = promo[asc]
    leg = [base[0] * curves[cv[0]][lvl - 1] + p.get(23, 0), base[1] * curves[cv[1]][lvl - 1] + p.get(24, 0),
           base[2] * curves[cv[2]][lvl - 1] + p.get(25, 0), p.get(8, 0)]
    for name, o, l in zip(['base_hp', 'base_atk', 'base_def', 'em'], vals, leg):
        out.append(rec('xiangling.%s@lv%d.a%d' % (name, lvl, asc), 'points', o, round(l, 4),
                       {'base_hp': 'engine/src/data.rs CharData xiangling (base x CURVES[22] + promo)', 'base_atk': 'engine/src/data.rs CharData xiangling (base x CURVES[32] + promo)', 'base_def': 'engine/src/data.rs CharData xiangling (base x CURVES[22] + promo)', 'em': 'engine/src/data.rs CharData xiangling (promo EM)'}[name],
                       round(l) == o, 'page shows integers; legacy rounded half-up', diff='%.3f' % (l - o)))

formula = [
    ('formula.general', '[(Base DMG x Base DMG Multiplier) + Additive Base DMG Bonus] x DEF mult x RES mult x DMG Bonus mult', 'sim.rs calc(): (mult*stat*(1+base_bonus)+flat) * (1+dmg%) * defmod * resmod * crit * amp', True, 'Damage Taken Multiplier (inside the DMG bonus sum on the page) not checked'),
    ('formula.def', '(Lc+100) / [(Lc+100) + (Le+100)(1-DEF reduced)(1-DEF ignored)]', 'sim.rs calc(): defmod, with defadj = -reduction', True, 'engine also caps defadj at +0.9; the page states no cap'),
    ('formula.res', 'RES<0: 1-RES/2; 0<=RES<75%: 1-RES; RES>=75%: 1/(1+4 RES)', 'sim.rs calc(): resmod', True, ''),
    ('formula.dmg_bonus', '(100% + general + elemental + ability DMG bonus + DMG taken) x amp x crit', 'sim.rs calc(): (1 + ele% + dmg%)', True, ''),
    ('formula.crit', '100% + CRIT DMG', 'sim.rs calc(): 1 + CD', True, ''),
    ('formula.amp.mult', 'Vaporize by Hydro / Melt by Pyro 200%; Vaporize by Pyro / Melt by Cryo 150%', 'sim.rs amp_mult 2.0 / 1.5', True, ''),
    ('formula.amp.em', 'EM x 2.78 / (EM + 1400)', 'sim.rs calc(): 2.78*em/(1400+em)', True, ''),
    ('formula.additive', 'Lv mult x (115% Aggravate | 125% Spread) x (1 + 5EM/(EM+1200) + bonus)', 'sim.rs: 1.15 / 1.25, 5*em/(1200+em)', True, 'page writes "Character LVL" for the level multiplier'),
    ('formula.transformative.em', 'EM x 16 / (EM + 2000)', 'sim.rs reaction_attack(): 16*em/(2000+em)', True, ''),
    ('formula.transformative.mult', 'Burgeon, Hyperbloom, Shatter 300%; Overloaded 275%; Bloom, Electro-Charged 200%; Superconduct 150%; Swirl 60%; Burning 25%', 'sim.rs: 3.0, 3.0, 3.0, 2.75, 2.0, 2.0, 1.5, 0.6, 0.25', True, ''),
]
for k, o, l, a, n in formula:
    out.append(rec(k, 'formula', o, l, 'engine/src/sim.rs', a, n, url=FORMULA, diff='none'))
lvl_tab = {60: 492.88, 70: 765.64, 80: 1077.44, 85: 1253.84, 90: 1446.85, 95: 1561.47, 100: 1674.81}
m = re.search(r'REACTION_LVL_BASE: &\[f64\] = &\[([^\]]+)\]', open(sys.argv[2].replace('data.rs', 'gcsimdata/combat.rs')).read())
rl = [float(x) for x in m.group(1).split(',')]
for L, v in lvl_tab.items():
    lv = rl[L - 1]
    out.append(rec('formula.level_multiplier@lv%d' % L, 'points', v, lv, 'engine/src/gcsimdata/combat.rs REACTION_LVL_BASE', round(lv, 2) == v, '', url=FORMULA, diff='%.4f' % (lv - v)))

out.append(rec('xiangling.enka.avatarId', 'id', None, 10000023, 'Gscouter internal/enka/data/character.dm.json (sub_id 2301)', None,
               'UNRESOLVED: no saved showcase in Gscouter has Xiangling; the one request made (UID 618285856, Enka API docs example) does not show her', evidence='unavailable', url=ENKA))

json.dump({'pilot': 'xiangling', 'observed': OBS, 'records': out,
           'ratio_derivation': talents['ratio_derivation']}, open(sys.argv[3], 'w'), indent=1)
agree = sum(1 for r in out if r['agree'] is True); dis = sum(1 for r in out if r['agree'] is False)
print('records', len(out), 'agree', agree, 'differ', dis)
from collections import Counter
print(Counter(r['key'].split('.')[0] + '.' + r['key'].split('.')[1].split('@')[0].split('[')[0] for r in out))
