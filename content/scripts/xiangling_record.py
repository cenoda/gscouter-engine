"""Assembles the layer 1 part of Xiangling's record once, from the legacy
draft and the step 0 pilot's official observations.

    cargo run -q --manifest-path content/Cargo.toml -- import xiangling \
      | python3 -I content/scripts/xiangling_record.py docs/pilot-xiangling/official.json

It prints the draft with an `official_rounded` observation added to each
talent row the pilot read (docs/pilot-xiangling/official.json), the official
scalars of the CHARS row, and the base stats at the breakpoints as check
values. The record was then completed by hand (the layer 1 scalars,
passives and constellations, and layer 2 from docs/pilot-xiangling/hidden.md)
and is edited by hand since: this script records how it began and is not
run again.
"""

import json
import re
import sys

official = json.load(open(sys.argv[1]))["records"]
draft = sys.stdin.read()

# pilot key -> record key
KEYS = {
    "charge": "nc",
    "plunge_collision": "collision",
    "plunge_low": "low_plunge",
    "plunge_high": "high_plunge",
}

rows = {}
stats = {}
for r in official:
    m = re.fullmatch(r"xiangling\.([a-z_]+(?:\[\d\])*)@lv(\d+)", r["key"])
    if m:
        name, lv = m.group(1), int(m.group(2))
        name = re.sub(r"^[a-z_]+", lambda x: KEYS.get(x.group(0), x.group(0)), name)
        rows.setdefault(name, {})[lv] = r["official"]
        continue
    m = re.fullmatch(r"xiangling\.(base_hp|base_atk|base_def|em)@lv(\d+)\.a(\d)", r["key"])
    if m:
        stats.setdefault(m.group(1), {})[(int(m.group(2)), int(m.group(3)))] = r["official"]

OBS = """
  [[value.obs]]
  evidence = "official_rounded"
  source = "hoyowiki"
  at = "{at}"
  shown = [{shown}]
"""

AT = {
    "attack": "Normal Attack table, {n}-Hit DMG",
    "nc": "Normal Attack table, Charged Attack DMG",
    "collision": "Normal Attack table, Plunge DMG",
    "low_plunge": "Normal Attack table, Low Plunge DMG",
    "high_plunge": "Normal Attack table, High Plunge DMG",
    "guoba_tick": "Guoba Attack table, Flame DMG",
    "pyronado_initial": "Pyronado table, {n}-Hit Swing DMG",
    "pyronado_spin": "Pyronado table, Pyronado DMG",
}

blocks = draft.split("\n[[value]]\n")
out = [blocks[0].replace('name = "xiangling"', 'name = "Xiangling"')]
for b in blocks[1:]:
    key = re.search(r'^key = "([^"]+)"', b, re.M).group(1)
    if key in rows:
        lv = rows[key]
        assert sorted(lv) == list(range(1, len(lv) + 1)), key
        base = re.match(r"[a-z_]+", key).group(0)
        n = int(re.search(r"\[(\d)\]", key).group(1)) + 1 if "[" in key else 0
        b = b.rstrip("\n") + "\n" + OBS.format(at=AT[base].format(n=n), shown=", ".join(f'"{lv[i]}"' for i in sorted(lv)))
    out.append(b)
text = "\n[[value]]\n".join(out)

BREAKPOINTS = [(1, 0), (20, 0), (20, 1), (40, 1), (40, 2), (50, 2), (50, 3), (60, 3), (60, 4), (70, 4), (70, 5), (80, 5), (80, 6), (90, 6)]
for stat in ["base_hp", "base_atk", "base_def", "em"]:
    shown = ", ".join(str(stats[stat][p]) for p in BREAKPOINTS)
    text += f"""
[[value]]
key = "stats.{stat}"
unit = "points"
layer = 1
check = true
by = "breakpoint"
stat = "{stat}"
note = "Checks char.base, char.curves, char.promo and char.em at the 14 breakpoints the page shows"

  [[value.obs]]
  evidence = "official_rounded"
  source = "hoyowiki"
  at = "Ascension table, levels 1 to 90 before and after each ascension"
  shown = [{shown}]
"""
sys.stdout.write(text)
