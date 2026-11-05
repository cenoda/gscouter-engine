"""Compare Arlecchino's HoYoWiki talent tables (entry 5149) with the engine's
legacy_gcsim multipliers (engine/src/gamedata/chars/arlecchino.rs).
Usage: python3 -I compare_talents.py <repo root>"""
import re, sys
root = sys.argv[1]
page = open(root + '/docs/arlecchino/official/hoyowiki-entry5149.txt').read()
rs = open(root + '/engine/src/gamedata/chars/arlecchino.rs').read()

def legacy(name):
    m = re.search(r'pub static %s: [^=]+= (.*?);\n' % name, rs)
    return eval(m.group(1).replace('&', '').replace('[', '[').replace(']', ']'))

def row(label):
    m = re.search(r'^' + re.escape(label) + r'\t(.*)$', page, re.M)
    return m.group(1).split('\t')

def pct(cell, part=0):
    return float(re.split(r'[+/]', cell)[part].rstrip('%').strip()) / 100

atk = legacy('ATTACK')
checks = [('1-Hit DMG', lambda l: atk[0][0][l], 0), ('2-Hit DMG', lambda l: atk[1][0][l], 0),
          ('3-Hit DMG', lambda l: atk[2][0][l], 0), ('4-Hit DMG', lambda l: atk[3][0][l], 0),
          ('4-Hit DMG', lambda l: atk[3][1][l], 1), ('5-Hit DMG', lambda l: atk[4][0][l], 0),
          ('6-Hit DMG', lambda l: atk[5][0][l], 0), ('Charged Attack DMG', lambda l: legacy('CHARGE')[l], 0),
          ('Plunge DMG', lambda l: legacy('COLLISION')[l], 0), ('Low/High Plunge DMG', lambda l: legacy('LOW_PLUNGE')[l], 0),
          ('Low/High Plunge DMG', lambda l: legacy('HIGH_PLUNGE')[l], 1),
          ('Masque of the Red Death Increase', lambda l: legacy('MASQUE')[l], 0),
          ('Spike DMG', lambda l: legacy('SKILL_SPIKE')[l], 0), ('Cleave DMG', lambda l: legacy('SKILL_FINAL')[l], 0),
          ('Blood-Debt Directive DMG', lambda l: legacy('SKILL_SIGIL')[l], 0), ('Skill DMG', lambda l: legacy('BURST')[l], 0)]
n = bad = 0
for label, get, part in checks:
    for lvl, cell in enumerate(row(label)):
        off, leg = pct(cell, part), get(lvl)
        n += 1
        # the page shows one decimal of a percentage: agree within half a unit of 0.1%
        if abs(off - leg) > 0.0005 + 1e-9:
            bad += 1
            print('DIFF', label, part, 'Lv.%d' % (lvl + 1), 'official', cell, 'legacy', leg)
print('%d values compared, %d differ beyond display rounding' % (n, bad))
