"""Recompute every hit of fixture F1 by hand from official values and the
official damage formula, and compare with gcsim's hit log.
Usage: python3 -I hand_f1.py <f1-hits-gcsim.tsv>

Sources (labels as in docs/arlecchino/report.md):
- official: HoYoWiki entry 5149 (observed 2026-11-05): Lv.90 base ATK 342,
  base HP 13103; talent Lv.9 multipliers; Masque Lv.9 221.2%; Directive
  65% / Due 130% of max HP; 7.5% of the Bond consumed per normal-attack hit;
  Masque from 30% of max HP; passive Pyro DMG +40%; burst heal
  150% Bond + 150% ATK.
- official: HoYoWiki Damage Formula page (entry 5654, read by the Xiangling
  pilot): DEF multiplier (Lc+100)/((Lc+100)+(Le+100)), RES multiplier 1-RES.
- legacy_gcsim: Favonius Lance base ATK 565 at 90/90 (not read here).
"""
import sys
BASE_ATK, WEAPON_ATK, BASE_HP = 342, 565, 13103
ATK = (BASE_ATK + WEAPON_ATK) * (1 + 0.466) + 311
MAXHP = BASE_HP + 4780
MULT = {'Normal 0': .873, 'Normal 1': .957, 'Normal 2': 1.201, 'Normal 3': .682, 'Normal 4': 1.286,
        'Normal 5': 1.569, 'Charge': 1.668, 'All is Ash (Spike)': .252, 'All is Ash (Cleave)': 2.271,
        'Blood Debt Directive': .541, 'Balemoon Rising': 6.297}
MASQUE = 2.212
DEF = (90 + 100) / ((90 + 100) + (100 + 100))
RES = 1 - 0.1
bond = 0.0
worst = 0.0
print('frame abil official gcsim diff')
for line in open(sys.argv[1]):
    f = line.rstrip('\n').split('\t')
    if f[0] == 'frame':
        continue
    if f[2] == 'BOND':
        continue
    abil = f[2]
    if abil == 'Charge' and bond == 0:
        bond = 1.30  # the charged attack absorbed a Blood-Debt Due (A1: 130%, limit 145%)
    masque = bond >= 0.30
    pyro = abil not in MULT or not abil.startswith(('Normal', 'Charge')) or masque
    flat = MASQUE * bond * ATK if (masque and abil.startswith('Normal')) else 0
    dmg = (MULT[abil] * ATK + flat) * (1 + (0.40 if pyro else 0)) * DEF * RES
    if masque and abil.startswith('Normal'):
        bond *= 1 - 0.075
    if abil == 'Balemoon Rising':
        pass
    if f[0] == '1260':
        bond = 0
    got = float(f[-1])
    d = dmg / got - 1
    worst = max(worst, abs(d))
    print(f[0], abil, round(dmg, 1), got, '%+.3f%%' % (100 * d))
    if abil == 'Balemoon Rising':
        # the heal (150% Bond + 150% ATK) exceeds the Bond and clears it
        bond = 0
print('largest difference %.3f%% (multipliers are read at the page\'s display precision)' % (100 * worst))
