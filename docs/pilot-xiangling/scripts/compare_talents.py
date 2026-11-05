"""Compare HoYoWiki's rendered Xiangling talent tables with the legacy arrays.

usage: python3 -I talents.py <read.txt> <xiangling.rs> <out.json>
"""
import json, re, sys
from decimal import Decimal, ROUND_HALF_UP

text = open(sys.argv[1]).read()
rs = open(sys.argv[2]).read()

def arr(name):
    m = re.search(r'pub static ' + name + r': [^=]+= (.*?);\n', rs)
    s = m.group(1)
    # flatten nesting into python lists
    s = s.replace('&[', '[')
    return json.loads(s)

ATTACK = arr('ATTACK')
L = {
    'NC': arr('NC'), 'COLLISION': arr('COLLISION'), 'LOW_PLUNGE': arr('LOW_PLUNGE'),
    'HIGH_PLUNGE': arr('HIGH_PLUNGE'), 'GUOBA_TICK': arr('GUOBA_TICK'),
    'PYRONADO_INITIAL': arr('PYRONADO_INITIAL'), 'PYRONADO_SPIN': arr('PYRONADO_SPIN'),
}

tabs = re.split(r'\n===== ', text)
def rows(tab_idx):
    t = [x for x in tabs if x.startswith('TALENT TAB %d' % tab_idx)][0]
    out = {}
    for line in t.split('\n'):
        parts = line.split('\t')
        if len(parts) > 2:
            out[parts[0].strip()] = [p.strip() for p in parts[1:]]
    return out

na, sk, bu = rows(0), rows(1), rows(2)

def disp_round(v, shown):
    """Round a legacy fraction v the way the shown percentage is written."""
    pct = Decimal(repr(v * 100))
    d = len(shown.split('.')[1]) if '.' in shown else 0
    q = Decimal(1).scaleb(-d)
    return str(pct.quantize(q, rounding=ROUND_HALF_UP))

records = []
def cmp(key, label, shown_list, legacy_list, part=None):
    for i, shown in enumerate(shown_list):
        s = shown.rstrip('%')
        if part is not None:
            s = re.split(r'[+*/]', s)[part].rstrip('%')
        lv = legacy_list[i]
        r = disp_round(lv, s)
        agree = Decimal(r) == Decimal(s)
        records.append({'key': key, 'row': label, 'level': i + 1, 'official': s + '%', 'legacy': lv,
                        'legacy_rounded': r + '%', 'agree': agree,
                        'diff_pct_points': float(Decimal(repr(lv * 100)) - Decimal(s))})

# normal attack
for n in range(5):
    label = '%d-Hit DMG' % (n + 1)
    for part in range(len(ATTACK[n])):
        cmp('xiangling.attack[%d][%d]' % (n, part), label, na[label], ATTACK[n][part][:11], part if n == 2 else (0 if n == 3 else None))
cmp('xiangling.charge', 'Charged Attack DMG', na['Charged Attack DMG'], L['NC'])
cmp('xiangling.plunge_collision', 'Plunge DMG', na['Plunge DMG'], L['COLLISION'])
cmp('xiangling.plunge_low', 'Low/High Plunge DMG', na['Low/High Plunge DMG'], L['LOW_PLUNGE'], 0)
cmp('xiangling.plunge_high', 'Low/High Plunge DMG', na['Low/High Plunge DMG'], L['HIGH_PLUNGE'], 1)
cmp('xiangling.guoba_tick', 'Flame DMG', sk['Flame DMG'], L['GUOBA_TICK'])
for i in range(3):
    label = '%d-Hit Swing DMG' % (i + 1)
    cmp('xiangling.pyronado_initial[%d]' % i, label, bu[label], L['PYRONADO_INITIAL'][i])
cmp('xiangling.pyronado_spin', 'Pyronado DMG', bu['Pyronado DMG'], L['PYRONADO_SPIN'])

bad = [r for r in records if not r['agree']]
print('compared', len(records), 'agree', len(records) - len(bad), 'differ', len(bad))
for r in bad:
    print(' ', r['key'], 'Lv', r['level'], 'official', r['official'], 'legacy', r['legacy'], '->', r['legacy_rounded'])

# Derived: the growth ratio r_n = M_n / M_1 per level, as an interval that
# every official row of a talent allows given its display rounding.
def iv(shown):
    s = shown.rstrip('%')
    d = len(s.split('.')[1]) if '.' in s else 0
    h = Decimal(5).scaleb(-d - 1)
    v = Decimal(s)
    return v - h, v + h

def ratio_intervals(cols):  # cols: list of lists of shown strings (rows)
    n = len(cols[0]); out = []
    for i in range(n):
        lo, hi = Decimal(0), Decimal(100)
        for r in cols:
            a, b = iv(r[i]); c, d = iv(r[0])
            lo = max(lo, a / d); hi = min(hi, b / c)
        out.append((lo, hi))
    return out

def split_rows(tab, labels):
    rows_ = []
    for lab in labels:
        vals = tab[lab]
        k = len(re.split(r'[+/]', vals[0].split('*')[0]))
        for p in range(k):
            rows_.append([re.split(r'[+/]', v.split('*')[0])[p] for v in vals])
    return rows_

groups = {
    'normal+charged (Lv1-11)': (split_rows(na, ['1-Hit DMG', '2-Hit DMG', '3-Hit DMG', '4-Hit DMG', '5-Hit DMG', 'Charged Attack DMG']),
                               [ATTACK[0][0], ATTACK[1][0], ATTACK[2][0], ATTACK[3][0], ATTACK[4][0], L['NC']]),
    'plunge (Lv1-11)': (split_rows(na, ['Plunge DMG', 'Low/High Plunge DMG']),
                               [L['COLLISION'], L['LOW_PLUNGE'], L['HIGH_PLUNGE']]),
    'skill+burst (Lv1-13)': (split_rows(sk, ['Flame DMG']) + split_rows(bu, ['1-Hit Swing DMG', '2-Hit Swing DMG', '3-Hit Swing DMG', 'Pyronado DMG']),
                             [L['GUOBA_TICK'], L['PYRONADO_INITIAL'][0], L['PYRONADO_INITIAL'][1], L['PYRONADO_INITIAL'][2], L['PYRONADO_SPIN']]),
}
deriv = {}
for g, (rows_, legacy_rows) in groups.items():
    ivs = ratio_intervals(rows_)
    ok = all(lo <= Decimal(repr(lr[i])) / Decimal(repr(lr[0])) <= hi for lr in legacy_rows for i, (lo, hi) in enumerate(ivs))
    deriv[g] = {'ratio_interval': [[float(lo), float(hi)] for lo, hi in ivs],
                'legacy_ratios_inside': ok,
                'legacy_ratios_row0': [lr_i / legacy_rows[0][0] for lr_i in legacy_rows[0][:len(ivs)]]}
    print(g, 'legacy ratios inside official intervals:', ok)
    for i, (lo, hi) in enumerate(ivs):
        print('  Lv%-2d [%.5f, %.5f] legacy %.5f' % (i + 1, lo, hi, legacy_rows[0][i] / legacy_rows[0][0]))
json.dump({'talent_records': records, 'ratio_derivation': deriv}, open(sys.argv[3], 'w'), indent=1, default=str)
