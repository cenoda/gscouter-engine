# Content records

> Status: In use for step 1 of Phase 6 (`docs/independence.md` §5, `docs/independence-roadmap.md` milestone 1): the format, its validator and its converter exist and are proven on Xiangling; no engine data is generated from records yet · Last updated: 2026-11-05

A content record holds a character's values, each with the observations that give it and where each came from. `gsc` (in `content/`) checks a record and converts it to the engine's data: a tables module in the shape of `engine/src/gamedata/chars/<name>.rs` and a `CharData` row for `engine/src/data.rs`. Neither needs gcsim's source or tools. How to add a character with it is `docs/porting.md` §5.

## 1. Where things are

| Path | What |
|---|---|
| `content/records/chars/<key>.toml` | The records, one per character |
| `content/src/` | `gsc`: the format's parser (`toml.rs`), the record model (`record.rs`), the validator (`validate.rs`), the converter (`convert.rs`), the importer of legacy data (`import.rs`), the engine's catalogs read from its source (`catalog.rs`), Rust data in and out (`rustdata.rs`) |
| `content/tests/xiangling.rs` | The proof on Xiangling and the round trip of every character (§7) |
| `content/scripts/xiangling_record.py` | How Xiangling's record began, from the step 0 pilot; not run again |

Commands, from anywhere in the repository:

```
cd content
cargo run -q -- check records/chars/xiangling.toml        # validate
cargo run -q -- status records/chars/xiangling.toml       # provisional / high uncertainty
cargo run -q -- convert records/chars/xiangling.toml --out <dir> [--select legacy|independent]
cargo run -q -- import <character>                        # a draft record from legacy data
cargo test                                                # CI runs it (compare.yml, part 0)
```

`convert --out` writes `<module>.rs` (the tables module) and `<module>.row.rs` (the row); both start with a header naming `gsc convert` and the record.

**Why TOML, and only a subset.** The engine has no Cargo dependencies and the converter keeps none either, so the parser is the repository's own (`content/src/toml.rs`, about 500 lines with its tests). TOML is editable by hand, takes comments, and any TOML parser reads a record the same way (Python's `tomllib` reads Xiangling's to the same 112 values). The subset: comments, `[table]` and `[[array.of.tables]]` headers, basic and literal strings, integers, decimal floats with an exponent, booleans, arrays over several lines, inline tables. Not allowed, as errors: dotted keys in assignments, multi-line strings, dates, hex, `inf`/`nan`, underscores in numbers. JSON was the alternative (simpler to parse); it has no comments, and a record is mostly notes on sources.

## 2. A record

```toml
format = 1
kind = "character"          # weapons and sets: §6
key = "xiangling"           # the canonical key in data.rs CHAR_NAMES
name = "Xiangling"
module = "xiangling"        # the tables module's file name
select = "legacy"           # which observation the converter takes (§4)

[source.hoyowiki]
kind = "official_page"
title = "HoYoWiki, Xiangling (entry 50)"
url = "https://wiki.hoyolab.com/pc/genshin/entry/50?lang=en-us"
observed = "2026-11-05"
game_version = "live on 2026-11-05 (page revision 2025-12-08T07:14:18Z)"

[items]                     # the tables module's items, in the order written out
ATTACK = { type = "&[&[&[f64]]]", len = [1, 1, 2, 4, 1] }
CHARGE_HITMARK = "i32"

[[value]]
key = "attack[0][0]"        # the content key, unique in the record
unit = "atk_frac"
layer = 1
target = "ATTACK[0][0]"     # where it goes: an item, an index into one, or char.<field>
by = "talent_level"         # one number per talent level, 1 to 15

  [[value.obs]]
  evidence = "legacy_gcsim"
  source = "legacy_tables"
  data = [0.42054, 0.45477, ...]

  [[value.obs]]
  evidence = "official_rounded"
  source = "hoyowiki"
  at = "Normal Attack table, 1-Hit DMG"
  shown = ["42.1%", "45.5%", ...]   # levels 1 to 11, as the page shows them
```

**Top level**: `format` (1), `kind`, `key`, `name`, `module`, `select`, then `[source.<id>]`, `[items]` and `[[value]]`. An unknown field anywhere is an error.

**Sources** (`[source.<id>]`): `kind`, `title`, and `url` or `video`; `observed` (YYYY-MM-DD) and `game_version` are defaults for the observations that cite it. Kinds:

| Kind | Given by | For |
|---|---|---|
| `official_page`, `official_text` | URL | `official`, `official_rounded` |
| `video` | `video` URL; each observation gives `at`, a timestamp `m:ss` or a range | `measured`, `stated` |
| `sheet`, `article`, `community` | URL | `measured`, `stated` |
| `project` | a file of this repository | the standard estimates (`docs/independence.md` §4) |
| `legacy_gcsim` | a file of this repository | `legacy_gcsim` only |

**Items** (`[items]`): each item of the tables module with its Rust type (`f64`, `i32`, `&str`, `bool`, `FrameTable`, and `&[T]` of those), optionally with `len`: a slice's length, or the lengths of its rows. Scalars are written as `pub const`, the rest as `pub static`, in the order of `[items]`.

**Values** (`[[value]]`):

| Field | |
|---|---|
| `key` | The content key: lowercase letters, digits and `_ . [ ] -` |
| `unit` | §3 |
| `layer` | 1: what the game shows; 2: what it does not (ICD, durability, hitlag, strike, hitboxes, particles, frames); 0: an engine identifier (unit `id`) |
| `field` | For layer 2, required: `frames`, `hitmark`, `icd_tag`, `icd_group`, `durability`, `hitlag`, `strike`, `poise`, `hitbox`, `particles`, `snapshot`, `geometry`, `count`, `timing`, `energy`, `cooldown`, `duration` |
| `target` | `ITEM`, `ITEM[i]`, `ITEM[i][j]`…, or `char.<field>` of `CharData` (`base`, `curves`, `em`, `promo`, `element`, `weapon_class`, `body`, `zone`, `rarity`, `energy_max`, `normal_hit_num`, `cons`, `anim_delay`) |
| `engine` | Where the engine's logic holds it, when no table does (`engine/src/chars/xiangling.rs TASK_SPIN_START: step_by(73)`) |
| `check` | `true` for a value kept only to check others (the base stats at the breakpoints) |
| `by` | `talent_level` (one number per level, 1 to 15) or `breakpoint` (the 14 points Lv.1, 20, 20+, 40, 40+ … 80+, 90, with `stat` = `base_hp`, `base_atk`, `base_def` or `em`) |
| `unresolved` | Why no public source gives it yet |
| `sensitivity` | `[[value.sensitivity]]`: `alternative`, `team` and `char` (DPS change in percent), `basis` |
| `note` | Free text |

A value needs `target`, `engine` or `check`. `char.` fields take names, not gcsim's numbers: `element = "pyro"`, `weapon_class = "polearm"`, `body = "girl"`, `zone = "liyue"`, `cons = { normal = -1, skill = 5, burst = 3 }` (the constellation that raises each talent by 3), `anim_delay = { none = 0, yelan_n0 = 0, xingqiu_n0 = 7 }` (gcsim's AnimationDelayKey), `promo = [{ level = 20 }, { level = 40, base_hp = 681.15454, base_def = 41.895, base_atk = 14.102028 }, …]` (the stats in the order written). A frame table is `{ length = 20, skill = 12, … }`: every action cancels at `length` unless named (`skill`, `burst`, `attack`, `charge`, `high_plunge`, `low_plunge`, `aim`, `dash`, `jump`, `swap`, `walk`).

**Observations** (`[[value.obs]]`): `evidence`, `source`, then `data` (in the value's unit) or `shown` (as the source shows it: `"42.1%"`, `"12.0s"`, `"1U"`, `912`). Optional: `at` (a section, table or timestamp), `observed` and `game_version` (override the source's), `first` (the level of the first number, `by = "talent_level"`), `tolerance` (how far the true value may be, for `data`), `differs` (why it disagrees with another observation beyond rounding), `note`. By evidence:

| Evidence | Meaning | Requires |
|---|---|---|
| `official` | An exact official value or statement | an official source |
| `official_rounded` | An official value at display precision | an official source, `shown` |
| `derived` | Computed from other values | `formula`, `inputs` (value keys, `key@lv3` allowed) |
| `measured` | A public test with its evidence | a video, sheet, article or community source |
| `stated` | A community statement without linked evidence | a source |
| `estimated` | An estimate | `basis`; `analogy = "<character>"` when it is taken from one (whether that is allowed for hitlag and hitboxes is open, §8) |
| `legacy_gcsim` | Kept from the gcsim-generated data under the legacy exception | a `legacy_gcsim` source |
| `assigned` | A name the engine chose (a mod key): not a game value | unit `id`; no source |

Every observation resolves to a content key, a unit, a game version, a source (URL, or video and timestamp), an observation date and an evidence kind: on itself or from its source. The validator rejects one that does not.

## 3. Units

| Unit | Values |
|---|---|
| `atk_frac`, `hp_frac`, `def_frac`, `em_frac` | A multiplier of a stat, 0 to 100 (1.0 = 100%); `shown` as `"42.1%"` |
| `frac` | A fraction, −10 to 10 (a buff, a resistance change); `shown` as `"15%"` |
| `frames` | Integers at 60 fps; frame tables |
| `s` | Seconds; `shown` as `"12.0s"` |
| `points` | Stat points (base HP, ATK, DEF, EM) |
| `energy`, `stamina`, `particles`, `poise` | As named |
| `durability` | gcsim units (25 = 1U); `shown` as `"1U"` |
| `count` | Integers (hits, stacks) |
| `m`, `deg` | Game metres; degrees |
| `hitlag_s`, `factor` | Hitlag halt time in seconds, 0 to 1; a time scale, 0 to 1 |
| `rarity`, `level`, `constellation` | Integers 1–5, 1–100, −1–6 |
| `curve` | A growth curve id of the engine's `CURVES` |
| `stat_bonus` | `char.promo`'s list |
| `icd_tag`, `icd_group` | A name of the engine's ICD tags or groups (`icd.rs`, lowercase, without `TAG_`/`GROUP_`): `normal_attack`, `pole_extra_attack`, `none` |
| `element`, `weapon_class`, `body`, `zone`, `strike` | A name from the set the validator lists |
| `id`, `text` | Any string: an engine identifier; a description or condition as the source words it |
| `unclassified` | Any value, with `legacy_gcsim` observations only: what `gsc import` could not classify |

## 4. Selection and status

The converter takes one observation per value, or per talent level for `by = "talent_level"`, by the record's `select` (or `--select`):

- **`legacy`**: `legacy_gcsim`, `assigned`, `official`, `measured`, `derived`, `official_rounded`, `stated`, `estimated`. Existing content keeps its data (`docs/independence.md` §4).
- **`independent`**: `official`, `measured`, `derived`, `official_rounded`, `stated`, `estimated`, `assigned`, and `legacy_gcsim` last, only where nothing else covers a value. This is new content's selection, and the one `status` reports on.

Two observations of the same kind: the first written. A talent row must hold all 15 levels under both selections, as the engine's tables do; levels past the official page's are `derived` from the shared growth ratios or `estimated`, so they are marked.

**Status** (`gsc status`, `validate::status`) follows `docs/independence.md` §7.2–7.3, under the independent selection, over the values the engine uses (not `check` values):

- **high_uncertainty**: a value whose taken observation is not independent evidence (`official`, `official_rounded`, `derived`, `measured`) has a sensitivity above 5% for the team or the character.
- **provisional**: otherwise, any value taken from `stated`, `estimated` or `legacy_gcsim`, or `unresolved`.
- **independent**: neither.

Each list is printed: the values with high uncertainty and their alternative, legacy only (with levels), estimated or stated, unresolved.

## 5. What the validator checks

`gsc check` and every `convert`:

- **Structure**: the fields above, no unknown ones, types; `format` 1.
- **References**: the key is canonical in the engine's key table (`data.rs` `CHAR_NAMES`; an alias or a new name is an error: the name is added to the key table first); sources exist; repository paths exist; targets name declared items or `char.` fields; `derived` inputs are values of the record; `analogy` names a character; curve ids exist in `CURVES`; ICD tags and groups exist in `icd.rs`; stats are known.
- **Units and ranges**: every leaf by its unit (§3), integers where required, frame tables' action names, talent rows within 15 levels, breakpoint values 14 numbers.
- **Provenance**: as §2's tables: the source kind fits the evidence, dates are YYYY-MM-DD, a game version and an observation date resolve, a video gives its timestamp, each evidence kind has what it requires.
- **Agreement**: an `official_rounded` value and every other observation of it with full numbers agree within half the last digit shown, at each level, unless one says why with `differs` (the pilot's 301 comparisons, made by the validator). The official base stats at the 14 breakpoints agree with `char.base`, `char.curves`, `char.promo` and `char.em` under both selections (`base × CURVES[curve][level − 1] + promo`, as `engine/src/profile.rs` `char_base`).
- **Conversion**: both selections convert; every item is fully set (a row missing is an error, also against `len`), nothing is set twice, every `char.` field is set, and the values have their items' types.

The engine's catalogs are read from its source (`catalog.rs`), from `engine/src/gamedata` or `engine/src/gcsimdata`, whichever exists.

## 6. Weapons and sets

Not converted yet; the validator rejects `kind = "weapon"` and `"set"`. The format fits them as it is: a weapon's tables module (`gamedata/weapons/<module>.rs`) has the same items, and its `WeaponData` row (`class`, `rarity`, `props` as `(stat, base, curve)`, `promo`) becomes `weapon.<field>` targets as `CharData`'s are `char.` ones, with refinement values `by = "refinement"`. A set has a tables module and its `SETS` row. Each needs its row type in `convert.rs` and its catalog in `catalog.rs`.

## 7. The proof on Xiangling

`content/records/chars/xiangling.toml` (`select = "legacy"`) has 112 values and 11 sources: the legacy tables and row, the official values of the step 0 pilot (`docs/pilot-xiangling/official.json`: talent rows at levels 1 to 11 or 13, the energy, the constellations that raise talents, the base stats at the breakpoints, passives and constellations), and layer 2 from `docs/pilot-xiangling/hidden.md` (measured frames, statements, standard estimates, 35 unresolved values, and the sensitivities of its §3). `content/tests/xiangling.rs` shows:

- **The legacy selection reproduces her engine data exactly**: the converted tables module equals `engine/src/gamedata/chars/xiangling.rs` byte for byte after its header, and the row equals her line of `data.rs`. The engine's data is not changed on this branch.
- **The independent selection differs only within display rounding**: 173 of the module's 474 numbers change, all talent multipliers, each by at most half the last digit the page shows (at most 0.5 percentage points, at most 0.38% of the value); the row does not change (80 energy, the constellations and the hit count are exact). Levels the page does not show (normal attack 12–15, skill and burst 14–15) stay legacy and are listed by `status`.
- **The validator does what the pilot did by hand**: every official value agrees with legacy at display precision, and the breakpoints check the legacy base stats and curves; a changed digit fails (`the_validator_names_what_is_wrong`).
- **Status as new content: `high_uncertainty`**, from the Pyronado spin's ICD (stated, against the standard estimate: team −4.9%, Xiangling −12%), its hits per rotation (unmeasured for Gscouter's target: up to +28% / +57%), its C0 hit count (stated), Guoba's particles (stated: −27% on an energy-starved build) and breath interval (stated: Xiangling +7.2%), as `hidden.md` §5 concluded.
- **Every character's legacy data round-trips**: `gsc import` then `gsc convert` gives back all 129 rows and tables modules: 125 byte for byte; 4 (Clorinde, Gaming, Lyney, Sigewinne) with the same values in other float text (gendata printed Go's `1e-07` and broke a tie in the 17th digit to even).

## 8. Open questions

For the owner or the coordinator:

1. **Full-precision base stats for new content.** HoYoWiki shows base stats as integers. Rounded breakpoints fix neither `base` nor the ascension bonuses to full precision (an error of 0.5 at Lv.1 grows with the curve). Enka showcases carry base HP and DEF to float precision at the showcase's level (`docs/pilot-xiangling/official.md` §6): `base = fightProp / CURVES[curve][level − 1]` (minus the phase's bonus) is a `derived` value with the showcase as its input. That makes a new character's base stats wait for a showcase; ATK needs the weapon's unrounded base ATK.
2. **Talent levels past the page.** The format requires 15 levels, as the engine's tables have. For a new character, normal attack 12–15 and skill and burst 14–15 are unreachable but must be filled: `derived` by the family's shared ratio (the pilot found that plunges leave the normal-attack sequence at Lv.11), or the engine's tables could hold only reachable levels.
3. **Estimates by analogy** with an existing character for hitlag and hitboxes are written `estimated` with `analogy`; whether they are allowed is still the owner's (`docs/independence-roadmap.md`, milestone 0, carried forward).
4. **Values in the engine's logic.** A layer 2 value with `engine = …` is recorded and checked, not converted: the logic holds it (ICD tags, Guoba's lifetime, the spin period). New content should put such values in its tables module so the record decides them; the item types have no ICD tag or element yet.
5. **The key catalog.** The validator reads `CHAR_NAMES` from `data.rs`; once step 1's key catalog lands, it reads that instead.
