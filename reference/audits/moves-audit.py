"""Runs a moved-player audit fixture under gcsim (go run . -abil) and the
engine at 1,000 iterations, warmup 0, with the line marked "# audit
position" set to each position given, and lists every ability's hits and
damage in both. Usage: python3 audit.py <fixture> [x,y ...]"""
import json, subprocess, sys, os, re
REF=os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
GSE=os.environ.get('GSE', os.path.join(REF, '..', 'engine', 'target', 'release', 'gse'))
env=dict(os.environ)
def run(cfg):
    g=subprocess.run(['go','run','.','-config',cfg,'-iter','1000','-warmup','0','-j','4','-abil'],cwd=REF,capture_output=True,text=True,env=env)
    if g.returncode: print('GCSIM ERR', g.stderr[-500:]); return None,None
    e=subprocess.run([GSE,cfg,'1000','0','4'],capture_output=True,text=True)
    if e.returncode: print('GSE ERR', e.stdout[-500:], e.stderr[-500:]); return None,None
    return json.loads(g.stdout), json.loads(e.stdout)
def cmp(name,g,e):
    gt=g['team']['mean'] if isinstance(g['team'],dict) else g['team']
    print(f"## {name}: gcsim {gt:.4f} engine {e['team']['mean']:.4f}")
    keys=sorted(set(g['abils'])|set(e['abils']))
    bad=0
    for k in keys:
        a=g['abils'].get(k,[0,0]); b=e['abils'].get(k,[0,0])
        hd=abs(a[0]-b[0]); dd=abs(a[1]-b[1])/max(abs(a[1]),1e-9)
        flag='' if hd<1e-9 and dd<1e-9 else '  <-- DIFF'
        if flag: bad+=1
        print(f"  {k}: hits {a[0]} / {b[0]}  dmg {a[1]:.6f} / {b[1]:.6f}{flag}")
    print('  waits gcsim', g.get('waits'), 'engine', e.get('waits'))
    return bad
total=0
fixture=os.path.abspath(sys.argv[1]); positions=sys.argv[2:]
src=open(fixture).read()
for pos in positions or ['-']:
    if pos=='-':
        cfg=fixture; name=os.path.basename(fixture)
    else:
        txt=re.sub(r'set_player_pos\([^)]*\); # audit position', f'set_player_pos({pos}); # audit position', src)
        cfg=f'/tmp/audit-{os.path.basename(fixture)}-{pos}.txt'; open(cfg,'w').write(txt); name=f'{os.path.basename(fixture)} @ {pos}'
    g,e=run(cfg)
    if g is None: total+=1; continue
    total+=cmp(name,g,e)
print('TOTAL DIFFS', total)
