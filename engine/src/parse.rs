//! The versioned parse API: what a config's setup lines say, as JSON, for
//! Gscouter's input path (assembling configs, reading a party, presets),
//! which read gcsim's `pkg/gcs/parser` before. `gse --parse <config>`,
//! `api::parse` and the Wasm module's `gse_parse` give it. It reads the
//! config with the engine's gcsl parser (`gcsl.rs`) and checks nothing
//! about simulation support: that is `gse --check`.
//!
//! ```text
//! {"parse_version": 1, "ok": true,
//!  "characters": [{"key", "level", "max_level", "ascension", "cons",
//!                  "talents": [attack, skill, burst], "params": {name: int},
//!                  "weapon": {"key", "level", "max_level", "refine", "params"} | null,
//!                  "sets": [{"key", "count", "params"}],
//!                  "stats": {stat: number}}],
//!  "active": key | null,
//!  "targets": [{"level", "resist": {element: number}, "hp", "pos": [x, y], "radius"}],
//!  "energy": {"mode": "once" | "every", "start", "end", "amount"} | null,
//!  "options": {"iterations", "duration", "swap_delay", "hitlag", "defhalt",
//!              "ignore_burst_energy", "damage_mode"},
//!  "hurt": bool,
//!  "errors": [{"class", "message"}]}
//!
//! {"parse_version": 1, "ok": false,
//!  "error": {"class", "line": n | null, "message"}}
//! ```
//!
//! - Keys are the catalog's (`gse --catalog`): an alias is resolved to its
//!   key. Characters are in the order of their first line; a character's
//!   later lines change the same character, as in gcsim (a second `char`
//!   line replaces its level, constellation and talents; `add stats` lines
//!   add up, stat by stat; a second `add set` of a set replaces its count;
//!   a second `add weapon` replaces the weapon).
//! - `talents` are the levels as written, without constellation bonuses.
//!   `ascension` is gcsim's `AvatarAsc` of the max level, null for a
//!   character without base data. `weapon` is null without `add weapon`.
//! - `sets` are sorted by key. `stats` are the sums of every `add stats`
//!   line, by gcsim's stat names (`hp`, `atk%`, `cr`, `pyro%`, ...), in
//!   gcsim's stat order, without zeros. Constant expressions are folded.
//! - `resist` lists the elements with a nonzero resistance, by gcsim's
//!   element names. A target's radius defaults to 1, as gcsim's does.
//! - `errors` are the checks of a whole config that gcsim reports in its
//!   `Errors` without failing the parse, in its order: classes
//!   `too_many_characters`, `no_active_character`, `too_many_set_items`,
//!   `active_not_in_team`, `no_targets`, `target_hp_missing`. A build's
//!   setup lines alone (no active character, no target) parse with errors.
//! - A config that does not parse gives `ok: false` and the first error:
//!   class `unknown_character`, `unknown_weapon`, `unknown_set`,
//!   `unsupported` (a form gcsim reads and the engine does not, such as
//!   random substats) or `syntax`; `line` is the 1-based line when known.
//!
//! Removing or renaming a field, or changing a value's meaning, raises
//! `parse_version`; adding a field does not.

use std::fmt::Write as _;

use crate::data;
use crate::gcsl::{self, ParsedConfig};
use crate::types::N_STATS;

pub const PARSE_VERSION: u32 = 1;

/// gcsim's attributes.StatTypeString, by the engine's stat index.
const STAT_NAMES: [&str; N_STATS] = [
    "n/a", "def%", "def", "hp", "hp%", "atk", "atk%", "er", "em", "cr", "cd", "heal", "pyro%", "hydro%", "cryo%", "electro%", "anemo%", "geo%", "dendro%", "phys%", "atkspd%", "dmg%", "", "base_hp", "base_atk", "base_def",
];
/// gcsim's element names, by the engine's Element index.
const ELEMENT_NAMES: [&str; 11] = ["none", "pyro", "hydro", "dendro", "electro", "cryo", "anemo", "geo", "physical", "frozen", "quicken"];

/// A config that did not parse.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub class: &'static str,
    pub line: Option<u32>,
    pub message: String,
}

/// Classifies one of the gcsl parser's messages.
fn error(msg: String) -> ParseError {
    let line = msg
        .strip_prefix("ln")
        .and_then(|r| r.split(':').next())
        .and_then(|n| n.parse().ok())
        .or_else(|| msg.strip_prefix("<target> bad token at line ").and_then(|r| r.split(':').next()).and_then(|n| n.parse().ok()));
    let body = match line {
        Some(_) if msg.starts_with("ln") => msg.split_once(": ").map_or(msg.as_str(), |x| x.1),
        _ => msg.as_str(),
    };
    let class = if body.starts_with("unknown character ") {
        "unknown_character"
    } else if body.starts_with("invalid weapon ") {
        "unknown_weapon"
    } else if body.starts_with("invalid set ") {
        "unknown_set"
    } else if body.starts_with("unsupported: ") {
        "unsupported"
    } else {
        "syntax"
    };
    ParseError { class, line, message: msg }
}

/// The checks gcsim's `Parse` reports in its `Errors`, in its order.
pub fn config_errors(c: &ParsedConfig) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if c.chars.len() > 4 {
        out.push(("too_many_characters", format!("config contains a total of {} characters; cannot exceed 4", c.chars.len())));
    }
    if c.active.is_none() {
        out.push(("no_active_character", "config does not contain active char".to_string()));
    }
    for ch in &c.chars {
        if ch.sets.iter().map(|s| s.1).sum::<i32>() > 5 {
            out.push(("too_many_set_items", format!("character {} has more than 5 total set items", gcsl::char_name(ch.key))));
        }
    }
    if let Some(a) = c.active {
        if !c.chars.iter().any(|x| x.key == a) {
            out.push(("active_not_in_team", format!("active char {} not found in team", gcsl::char_name(a))));
        }
    }
    if c.targets.is_empty() {
        out.push(("no_targets", "config does not contain any targets".to_string()));
    }
    if c.damage_mode {
        for (i, t) in c.targets.iter().enumerate() {
            if t.hp == 0.0 {
                out.push(("target_hp_missing", format!("damage mode is activated; target #{} does not have hp set", i + 1)));
            }
        }
    }
    out
}

/// gcsim's AvatarAsc: the last ascension whose max level the max level
/// reaches.
fn ascension(key: &str, max_level: i32) -> Option<i32> {
    let d = data::CHARS.iter().find(|d| d.key == key)?;
    let mut asc = 0;
    for (i, (max, _)) in d.promo.iter().enumerate() {
        if max_level >= *max as i32 {
            asc = i as i32;
        }
    }
    Some(asc)
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn num(out: &mut String, x: f64) -> Result<(), String> {
    if !x.is_finite() {
        return Err(format!("{x} is not a finite number"));
    }
    let _ = write!(out, "{x}");
    Ok(())
}

fn params(out: &mut String, ps: &[(String, i64)]) {
    out.push('{');
    for (i, (k, v)) in ps.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        string(out, k);
        let _ = write!(out, ": {v}");
    }
    out.push('}');
}

fn json(c: &ParsedConfig) -> Result<String, String> {
    let mut o = String::with_capacity(1024);
    let _ = write!(o, "{{\"parse_version\": {PARSE_VERSION}, \"ok\": true,\n\"characters\": [");
    for (i, ch) in c.chars.iter().enumerate() {
        o.push_str(if i > 0 { ",\n  " } else { "\n  " });
        let key = gcsl::char_name(ch.key);
        o.push_str("{\"key\": ");
        string(&mut o, key);
        let _ = write!(o, ", \"level\": {}, \"max_level\": {}, \"ascension\": ", ch.level, ch.max_level);
        match ascension(key, ch.max_level) {
            Some(a) => {
                let _ = write!(o, "{a}");
            }
            None => o.push_str("null"),
        }
        let [a, s, b] = ch.talents;
        let _ = write!(o, ", \"cons\": {}, \"talents\": [{a}, {s}, {b}], \"params\": ", ch.cons);
        params(&mut o, &ch.params);
        o.push_str(", \"weapon\": ");
        let w = &ch.weapon;
        if w.key == 0 {
            o.push_str("null");
        } else {
            o.push_str("{\"key\": ");
            string(&mut o, gcsl::weapon_name(w.key));
            let _ = write!(o, ", \"level\": {}, \"max_level\": {}, \"refine\": {}, \"params\": ", w.level, w.max_level, w.refine);
            params(&mut o, &w.params);
            o.push('}');
        }
        o.push_str(", \"sets\": [");
        let mut sets: Vec<(&str, i32, u16)> = ch.sets.iter().map(|&(k, n)| (gcsl::set_name(k), n, k)).collect();
        sets.sort_unstable();
        for (j, (name, count, k)) in sets.iter().enumerate() {
            if j > 0 {
                o.push_str(", ");
            }
            o.push_str("{\"key\": ");
            string(&mut o, name);
            let _ = write!(o, ", \"count\": {count}, \"params\": ");
            params(&mut o, ch.set_params.iter().find(|p| p.0 == *k).map_or(&[][..], |p| &p.1[..]));
            o.push('}');
        }
        o.push_str("], \"stats\": {");
        let mut first = true;
        for (j, v) in ch.stats.iter().enumerate() {
            if *v == 0.0 || j == 0 {
                continue;
            }
            if !first {
                o.push_str(", ");
            }
            first = false;
            string(&mut o, STAT_NAMES[j]);
            o.push_str(": ");
            num(&mut o, *v).map_err(|e| format!("{key}'s {}: {e}", STAT_NAMES[j]))?;
        }
        o.push_str("}}");
    }
    o.push_str("\n],\n\"active\": ");
    match c.active {
        Some(a) => string(&mut o, gcsl::char_name(a)),
        None => o.push_str("null"),
    }
    o.push_str(",\n\"targets\": [");
    for (i, t) in c.targets.iter().enumerate() {
        if i > 0 {
            o.push_str(", ");
        }
        let _ = write!(o, "{{\"level\": {}, \"resist\": {{", t.level);
        let mut first = true;
        for (e, v) in t.resist.iter().enumerate() {
            if *v == 0.0 {
                continue;
            }
            if !first {
                o.push_str(", ");
            }
            first = false;
            let _ = write!(o, "\"{}\": ", ELEMENT_NAMES[e]);
            num(&mut o, *v)?;
        }
        o.push_str("}, \"hp\": ");
        num(&mut o, t.hp)?;
        o.push_str(", \"pos\": [");
        num(&mut o, t.pos.0)?;
        o.push_str(", ");
        num(&mut o, t.pos.1)?;
        o.push_str("], \"radius\": ");
        num(&mut o, t.radius)?;
        o.push('}');
    }
    o.push_str("],\n\"energy\": ");
    let e = &c.energy;
    if e.active {
        let _ = write!(o, "{{\"mode\": \"{}\", \"start\": {}, \"end\": {}, \"amount\": {}}}", if e.once { "once" } else { "every" }, e.start, e.end, e.amount);
    } else {
        o.push_str("null");
    }
    let _ = write!(
        o,
        ",\n\"options\": {{\"iterations\": {}, \"duration\": ",
        c.iterations
    );
    num(&mut o, c.duration)?;
    let _ = write!(
        o,
        ", \"swap_delay\": {}, \"hitlag\": {}, \"defhalt\": {}, \"ignore_burst_energy\": {}, \"damage_mode\": {}}},\n\"hurt\": {},\n\"errors\": [",
        c.swap_delay, c.hitlag, c.defhalt, c.ignore_burst_energy, c.damage_mode, c.hurt
    );
    for (i, (class, msg)) in config_errors(c).iter().enumerate() {
        if i > 0 {
            o.push_str(", ");
        }
        let _ = write!(o, "{{\"class\": \"{class}\", \"message\": ");
        string(&mut o, msg);
        o.push('}');
    }
    o.push_str("]}\n");
    Ok(o)
}

fn error_json(e: &ParseError) -> String {
    let mut o = String::new();
    let _ = write!(o, "{{\"parse_version\": {PARSE_VERSION}, \"ok\": false,\n\"error\": {{\"class\": \"{}\", \"line\": ", e.class);
    match e.line {
        Some(l) => {
            let _ = write!(o, "{l}");
        }
        None => o.push_str("null"),
    }
    o.push_str(", \"message\": ");
    string(&mut o, &e.message);
    o.push_str("}}\n");
    o
}

/// The parse of `src` as JSON (see the module's documentation): Ok when
/// it parsed, Err (also JSON) when it did not. Never panics.
pub fn parse(src: &str) -> Result<String, String> {
    let r = std::panic::catch_unwind(|| {
        let parsed = gcsl::Parser::new(src).parse_setup().map_err(error)?;
        json(&parsed).map_err(|m| ParseError { class: "syntax", line: None, message: m })
    });
    match r {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(error_json(&e)),
        Err(_) => Err(error_json(&ParseError { class: "syntax", line: None, message: "the parser failed on this input".into() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> String {
        parse(src).unwrap_or_else(|e| panic!("{e}"))
    }
    fn err(src: &str) -> String {
        parse(src).expect_err("parsed")
    }

    #[test]
    fn build_lines_resolve_aliases_and_add_stats() {
        let j = ok("ht char lvl=90/90 cons=1 talent=9,9,10;\n\
            ht add weapon=\"homa\" refine=1 lvl=90/90;\n\
            ht add set=\"cw\" count=4;\n\
            ht add stats hp=4780 atk=311 cr=0.25*1;\n\
            ht add stats cr=0.1 hp%=0.1;\n\
            ht add set=\"crimsonwitchofflames\" count=2;\n");
        assert!(j.contains("{\"key\": \"hutao\", \"level\": 90, \"max_level\": 90, \"ascension\": 6, \"cons\": 1, \"talents\": [9, 9, 10], \"params\": {}, \"weapon\": {\"key\": \"staffofhoma\", \"level\": 90, \"max_level\": 90, \"refine\": 1, \"params\": {}}, \"sets\": [{\"key\": \"crimsonwitchofflames\", \"count\": 2, \"params\": {}}], \"stats\": {\"hp\": 4780, \"hp%\": 0.1, \"atk\": 311, \"cr\": 0.35}}"), "{j}");
        assert!(j.contains("\"errors\": [{\"class\": \"no_active_character\""), "{j}");
        assert!(j.contains("{\"class\": \"no_targets\""), "{j}");
    }

    #[test]
    fn reference_config_parses_clean() {
        let j = ok(include_str!("../../configs/team-looped.txt"));
        assert!(j.contains("\"errors\": []}"), "{j}");
        assert!(j.starts_with("{\"parse_version\": 1, \"ok\": true,"));
    }

    #[test]
    fn errors_are_classified() {
        let e = err("foo char lvl=90/90 cons=0 talent=9,9,9;\n");
        assert!(e.contains("\"class\": \"unknown_character\", \"line\": 1,"), "{e}");
        let e = err("hutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add weapon=\"nope\" refine=1 lvl=90/90;\n");
        assert!(e.contains("\"class\": \"unknown_weapon\", \"line\": 2,"), "{e}");
        let e = err("hutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add set=\"nope\" count=4;\n");
        assert!(e.contains("\"class\": \"unknown_set\", \"line\": 2,"), "{e}");
        let e = err("hutao char lvl=90/90 cons=0 talent=9,9;\n");
        assert!(e.contains("\"class\": \"syntax\", \"line\": 1,"), "{e}");
        let e = err("hutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add stats random rarity=5;\n");
        assert!(e.contains("\"class\": \"unsupported\""), "{e}");
    }

    #[test]
    fn duplicate_characters_merge_as_in_gcsim() {
        let j = ok("hutao char lvl=80/90 cons=0 talent=9,9,9;\nht char lvl=90/90 cons=2 talent=1,2,3;\n");
        assert_eq!(j.matches("\"key\": \"hutao\"").count(), 1, "{j}");
        assert!(j.contains("\"level\": 90, \"max_level\": 90, \"ascension\": 6, \"cons\": 2, \"talents\": [1, 2, 3]"), "{j}");
    }

    #[test]
    fn whole_config_checks_follow_gcsim() {
        let five = "hutao char lvl=90/90 cons=0 talent=9,9,9;\nxingqiu char lvl=90/90 cons=0 talent=9,9,9;\nbennett char lvl=90/90 cons=0 talent=9,9,9;\nxiangling char lvl=90/90 cons=0 talent=9,9,9;\nsucrose char lvl=90/90 cons=0 talent=9,9,9;\nactive zhongli;\ntarget lvl=100 resist=0.1;\n";
        let j = ok(five);
        assert!(j.contains("\"errors\": [{\"class\": \"too_many_characters\", \"message\": \"config contains a total of 5 characters; cannot exceed 4\"}, {\"class\": \"active_not_in_team\", \"message\": \"active char zhongli not found in team\"}]"), "{j}");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        for src in ["\"", "{", "hutao", "hutao add", "target", "options iteration=", "\u{0}\u{1}", "fn (", "let x = ;", "1/0", "hutao char lvl=90/80;"] {
            let _ = parse(src);
        }
    }
}
