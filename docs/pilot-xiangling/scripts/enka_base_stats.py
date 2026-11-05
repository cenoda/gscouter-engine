import json, re, sys
d = json.load(open(sys.argv[1])); dm = json.load(open(sys.argv[2]))['data']; src = open(sys.argv[3]).read()
id2key = {}
for k, v in dm.items(): id2key.setdefault(v['id'], k)
curves = {int(m.group(1)): [float(x) for x in m.group(2).split(',')] for m in re.finditer(r'\((\d+), &\[([0-9., e-]+)\]\)', src[src.find('pub static CURVES'):])}
def chardata(key):
    m = re.search(r'CharData \{ key: "%s", base: \[([^\]]+)\], curves: \[([^\]]+)\], em: [^,]+, promo: &\[(.*?)\], element' % key, src)
    base = [float(x) for x in m.group(1).split(',')]; cv = [int(x) for x in m.group(2).split(',')]
    promo = []
    for pm in re.finditer(r'\((\d+), &\[(.*?)\]\)', m.group(3)):
        promo.append({int(a): float(b) for a, b in re.findall(r'\((\d+), ([0-9.e-]+)\)', pm.group(2))})
    return base, cv, promo
for a in d['avatarInfoList']:
    key = id2key.get(a['avatarId']); lvl = int(a['propMap']['4001']['val']); asc = int(a['propMap'].get('1002', {}).get('val', 0))
    base, cv, promo = chardata(key)
    p = promo[asc]
    leg = [base[0]*curves[cv[0]][lvl-1]+p.get(23,0), base[1]*curves[cv[1]][lvl-1]+p.get(24,0), base[2]*curves[cv[2]][lvl-1]+p.get(25,0)]
    fp = a['fightPropMap']
    w = [e for e in a['equipList'] if 'weapon' in e][0]
    watk = [x['statValue'] for x in w['flat']['weaponStats'] if x['appendPropId'] == 'FIGHT_PROP_BASE_ATTACK'][0]
    enka = [fp.get('1'), fp.get('4') - watk, fp.get('7')]
    print(a['avatarId'], key, 'lv', lvl, 'asc', asc, 'enka', ['%.4f' % x for x in enka], 'legacy', ['%.4f' % x for x in leg], 'rel', ['%.1e' % ((e-l)/l) for e, l in zip(enka, leg)])
