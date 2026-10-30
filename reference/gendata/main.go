// Command gendata writes the engine's data from gcsim, so that porting a
// character, weapon or set is only its logic:
//
//   - engine/src/data.rs from gcsim's catalog: the names gcsl accepts for
//     characters, weapons and sets; every character's and weapon's base
//     stats (curves and ascension stats); and what each character's
//     NewChar and Init set (element, weapon class, body, energy, normal
//     hit count, the constellations that raise skill and burst, animation
//     start delays).
//   - engine/src/gcsimdata/icd.rs: every ICD tag and group, with the groups'
//     timers and sequences.
//   - engine/src/gcsimdata/{chars,weapons,artifacts}/<package>.rs: the package
//     variables and constants of every character, weapon and artifact
//     package: talent multipliers, frame tables (as gcsim's init functions
//     build them), hitmarks, hitlag, ICD and the rest.
//
// The values are read from gcsim as it computes them: gendata copies the
// gcsim module the reference harness builds against into a temporary
// directory, adds to each package a file registering its variables, and
// runs dumper/main.go there. gcsim itself is never modified.
//
// Usage: go run ./gendata [-engine ../engine]
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"maps"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"

	"github.com/genshinsim/gcsim/pkg/catalog"
	"github.com/genshinsim/gcsim/pkg/core/info"
	"github.com/genshinsim/gcsim/pkg/core/keys"
	"github.com/genshinsim/gcsim/pkg/model"
	"github.com/genshinsim/gcsim/pkg/shortcut"
)

const gcsimModule = "github.com/genshinsim/gcsim"

// Packages whose variables are exported, by prefix.
var exportRoots = []string{"internal/characters", "internal/weapons", "internal/artifacts"}

// and these packages alone
var exportPackages = []string{"pkg/core", "pkg/core/attacks"}

func main() {
	engine := flag.String("engine", "../engine", "the engine's directory")
	keep := flag.Bool("keep", false, "keep the temporary copy of gcsim")
	flag.Parse()

	out, err := exec.Command("go", "list", "-m", "-f", "{{.Dir}}", gcsimModule).Output()
	check(err)
	src := strings.TrimSpace(string(out))
	tmp, err := os.MkdirTemp("", "gendata")
	check(err)
	if *keep {
		fmt.Fprintln(os.Stderr, "copy of gcsim:", tmp)
	} else {
		defer os.RemoveAll(tmp)
	}
	prepare(src, tmp)

	cmd := exec.Command("go", "run", "-tags", "gendata", "./cmd/zzgendata")
	cmd.Dir = tmp
	cmd.Env = append(os.Environ(), "GOFLAGS=-mod=mod", "GOWORK=off")
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	raw, err := cmd.Output()
	if err != nil {
		fmt.Fprintln(os.Stderr, stderr.String())
		check(err)
	}
	var d Dump
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	check(dec.Decode(&d))

	writeData(&d, filepath.Join(*engine, "src", "data.rs"))
	writeGen(&d, filepath.Join(*engine, "src", "gcsimdata"))
}

func check(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

// ------------------------------------------------------------ the copy

// prepare copies gcsim's go.mod, pkg and internal to dst and adds the
// export files, the registry and the dumper.
func prepare(src, dst string) {
	for _, f := range []string{"go.mod", "go.sum"} {
		b, err := os.ReadFile(filepath.Join(src, f))
		check(err)
		check(os.WriteFile(filepath.Join(dst, f), b, 0o644))
	}
	for _, dir := range []string{"pkg", "internal"} {
		check(filepath.WalkDir(filepath.Join(src, dir), func(p string, e fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			rel, _ := filepath.Rel(src, p)
			if e.IsDir() {
				return os.MkdirAll(filepath.Join(dst, rel), 0o755)
			}
			if strings.HasSuffix(p, "_test.go") {
				return nil
			}
			b, err := os.ReadFile(p)
			if err != nil {
				return err
			}
			return os.WriteFile(filepath.Join(dst, rel), b, 0o644)
		}))
	}
	check(os.MkdirAll(filepath.Join(dst, "pkg", "zzgendata"), 0o755))
	check(os.WriteFile(filepath.Join(dst, "pkg", "zzgendata", "zzgendata.go"), []byte(`package zzgendata

// Packages holds, by package path, a function returning the package's
// variables and constants by name.
var Packages = map[string]func() map[string]any{}
`), 0o644))
	dumper, err := os.ReadFile(filepath.Join("gendata", "dumper", "main.go"))
	check(err)
	check(os.MkdirAll(filepath.Join(dst, "cmd", "zzgendata"), 0o755))
	check(os.WriteFile(filepath.Join(dst, "cmd", "zzgendata", "main.go"), dumper, 0o644))

	var dirs []string
	for _, root := range exportRoots {
		check(filepath.WalkDir(filepath.Join(dst, root), func(p string, e fs.DirEntry, err error) error {
			if err == nil && e.IsDir() {
				dirs = append(dirs, p)
			}
			return err
		}))
	}
	for _, p := range exportPackages {
		dirs = append(dirs, filepath.Join(dst, p))
	}
	for _, dir := range dirs {
		inject(dst, dir)
	}
}

// inject adds to the package in dir a file registering its package-level
// variables and constants.
func inject(root, dir string) {
	ents, err := os.ReadDir(dir)
	check(err)
	fset := token.NewFileSet()
	pkg := ""
	var names []string
	for _, e := range ents {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".go") {
			continue
		}
		f, err := parser.ParseFile(fset, filepath.Join(dir, e.Name()), nil, parser.SkipObjectResolution)
		check(err)
		pkg = f.Name.Name
		for _, decl := range f.Decls {
			g, ok := decl.(*ast.GenDecl)
			if !ok || (g.Tok != token.VAR && g.Tok != token.CONST) {
				continue
			}
			for _, s := range g.Specs {
				for _, n := range s.(*ast.ValueSpec).Names {
					if n.Name != "_" {
						names = append(names, n.Name)
					}
				}
			}
		}
	}
	if pkg == "" || pkg == "main" {
		return
	}
	rel, _ := filepath.Rel(root, dir)
	rel = filepath.ToSlash(rel)
	var b strings.Builder
	fmt.Fprintf(&b, "package %s\n\nimport zzgendata \"%s/pkg/zzgendata\"\n\nfunc init() {\n\tzzgendata.Packages[%q] = func() map[string]any {\n\t\treturn map[string]any{\n", pkg, gcsimModule, rel)
	for _, n := range names {
		fmt.Fprintf(&b, "\t\t\t%q: %s,\n", n, n)
	}
	b.WriteString("\t\t}\n\t}\n}\n")
	check(os.WriteFile(filepath.Join(dir, "zz_gendata_export.go"), []byte(b.String()), 0o644))
}

// ------------------------------------------------------------ the dump

type Val struct {
	T string `json:"t"`
	V any    `json:"v"`
}

type Char struct {
	Key          string      `json:"key"`
	ID           int         `json:"id"`
	Package      string      `json:"package"`
	Error        string      `json:"error"`
	Element      int         `json:"element"`
	WeaponClass  int         `json:"weapon_class"`
	Body         int         `json:"body"`
	Zone         int         `json:"zone"`
	Rarity       int         `json:"rarity"`
	EnergyMax    json.Number `json:"energy_max"`
	NormalHitNum int         `json:"normal_hit_num"`
	NormalCon    int         `json:"normal_con"`
	SkillCon     int         `json:"skill_con"`
	BurstCon     int         `json:"burst_con"`
	AnimDelay    []int       `json:"anim_delay"`
}

type Item struct {
	Key     string `json:"key"`
	ID      int    `json:"id"`
	Package string `json:"package"`
}

type ICDGroup struct {
	Name  string        `json:"name"`
	Reset int           `json:"reset"`
	Ele   []json.Number `json:"ele"`
	Dmg   []json.Number `json:"dmg"`
}

type Dump struct {
	Packages  map[string]map[string]Val `json:"packages"`
	Chars     []Char                    `json:"chars"`
	Weapons   []Item                    `json:"weapons"`
	Sets      []Item                    `json:"sets"`
	ICDGroups []ICDGroup                `json:"icd_groups"`
	ICDTags   []string                  `json:"icd_tags"`
}

// ------------------------------------------------------------ Rust

// fl prints a float as a Rust f64 literal, with the shortest digits that
// read back as the same value.
func fl(v float64) string {
	s := strconv.FormatFloat(v, 'g', -1, 64)
	if !strings.ContainsAny(s, ".eE") {
		s += ".0"
	}
	return s
}

func num(n json.Number) float64 {
	f, err := strconv.ParseFloat(string(n), 64)
	check(err)
	return f
}

const header = "// Generated by reference/gendata from gcsim; do not edit.\n\n"

func writeFile(path, s string) {
	check(os.MkdirAll(filepath.Dir(path), 0o755))
	check(os.WriteFile(path, []byte(s), 0o644))
}

func writeData(d *Dump, path string) {
	var b strings.Builder
	w := func(f string, a ...any) { fmt.Fprintf(&b, f, a...) }
	w(header)
	w("#![allow(dead_code, clippy::all)]\n\n")

	names := func(title string, m map[string]int) {
		w("pub static %s: &[(&str, u16)] = &[\n", title)
		for _, k := range slices.Sorted(maps.Keys(m)) {
			w("    (%q, %d),\n", k, m[k])
		}
		w("];\n\n")
	}
	cm := map[string]int{}
	for k, v := range shortcut.CharNameToKey {
		cm[k] = int(v)
	}
	names("CHAR_NAMES", cm)
	wm := map[string]int{}
	for k, v := range shortcut.WeaponNameToKey {
		wm[k] = int(v)
	}
	names("WEAPON_NAMES", wm)
	sm := map[string]int{}
	for k, v := range shortcut.SetNameToKey {
		sm[k] = int(v)
	}
	names("SET_NAMES", sm)

	key := func(title string, m map[string]int, str func(int) string) {
		// canonical key name by id
		ids := map[int]bool{}
		for _, v := range m {
			ids[v] = true
		}
		w("pub static %s: &[(u16, &str)] = &[\n", title)
		for _, id := range slices.Sorted(maps.Keys(ids)) {
			w("    (%d, %q),\n", id, str(id))
		}
		w("];\n\n")
	}
	key("CHAR_KEYS", cm, func(i int) string { return keys.Char(i).String() })
	key("WEAPON_KEYS", wm, func(i int) string { return keys.Weapon(i).String() })
	key("SET_KEYS", sm, func(i int) string { return keys.Set(i).String() })

	curves := map[model.GrowCurveType]bool{}
	promo := func(p []*model.PromotionData) string {
		var s []string
		for _, v := range p {
			var props []string
			for _, a := range v.AddProps {
				props = append(props, fmt.Sprintf("(%d, %s)", info.ConvertProtoStat(a.PropType), fl(a.Value)))
			}
			s = append(s, fmt.Sprintf("(%d, &[%s])", v.MaxLevel, strings.Join(props, ", ")))
		}
		return "&[" + strings.Join(s, ", ") + "]"
	}

	w(`/// A character as gcsim's catalog and its NewChar and Init set it up.
pub struct CharData {
    pub key: &'static str,
    pub base: [f64; 3],
    pub curves: [u32; 3],
    pub em: f64,
    pub promo: &'static [(u32, &'static [(usize, f64)])],
    /// gcsim's attributes.Element, info.WeaponClass and info.BodyType.
    pub element: u8,
    pub weapon_class: u8,
    pub body: u8,
    pub rarity: u8,
    pub energy_max: f64,
    pub normal_hit_num: i32,
    /// The constellations that raise normal attack, skill and burst by 3.
    pub cons: [i32; 3],
    /// AnimationStartDelay by gcsim's AnimationDelayKey.
    pub anim_delay: [i32; 3],
    /// The package of its logic and data, under gcsimdata::chars.
    pub module: &'static str,
}

`)
	w("pub static CHARS: &[CharData] = &[\n")
	var failed []string
	for _, c := range d.Chars {
		k := keys.Char(c.ID)
		cd, ok := catalog.CharacterMap[k]
		if !ok {
			continue
		}
		if c.Error != "" {
			failed = append(failed, c.Key+": "+c.Error)
			continue
		}
		s := cd.Stats
		curves[s.HpCurve], curves[s.AtkCurve], curves[s.DefCruve] = true, true, true
		delay := [3]int{}
		copy(delay[:], c.AnimDelay)
		w("    CharData { key: %q, base: [%s, %s, %s], curves: [%d, %d, %d], em: %s, promo: %s, element: %d, weapon_class: %d, body: %d, rarity: %d, energy_max: %s, normal_hit_num: %d, cons: [%d, %d, %d], anim_delay: [%d, %d, %d], module: %q },\n",
			k.String(), fl(s.BaseHp), fl(s.BaseAtk), fl(s.BaseDef), s.HpCurve, s.AtkCurve, s.DefCruve, fl(s.ElementMastery), promo(s.PromoData),
			c.Element, c.WeaponClass, c.Body, c.Rarity, fl(num(c.EnergyMax)), c.NormalHitNum, c.NormalCon, c.SkillCon, c.BurstCon,
			delay[0], delay[1], delay[2], moduleName(c.Package))
	}
	w("];\n\n")
	for _, f := range failed {
		fmt.Fprintln(os.Stderr, "warning: character not set up:", f)
	}

	w("pub struct WeaponData {\n    pub key: &'static str,\n    pub class: u8,\n    pub rarity: u8,\n    pub props: &'static [(usize, f64, u32)],\n    pub promo: &'static [(u32, &'static [(usize, f64)])],\n    /// The package of its logic and data, under gcsimdata::weapons.\n    pub module: &'static str,\n}\n\n")
	w("pub static WEAPONS: &[WeaponData] = &[\n")
	for _, it := range d.Weapons {
		k := keys.Weapon(it.ID)
		wd, ok := catalog.WeaponMap[k]
		if !ok {
			continue
		}
		s := wd.BaseStats
		var props []string
		for _, p := range s.BaseProps {
			curves[p.Curve] = true
			props = append(props, fmt.Sprintf("(%d, %s, %d)", info.ConvertProtoStat(p.PropType), fl(p.InitialValue), p.Curve))
		}
		w("    WeaponData { key: %q, class: %d, rarity: %d, props: &[%s], promo: %s, module: %q },\n",
			k.String(), info.ConvertWeaponClass(wd.WeaponClass), wd.Rarity, strings.Join(props, ", "), promo(s.PromoData), moduleName(it.Package))
	}
	w("];\n\n")

	w("/// The artifact sets gcsim implements, and the package of each under gcsimdata::artifacts.\n")
	w("pub static SETS: &[(&str, &str)] = &[\n")
	for _, it := range d.Sets {
		w("    (%q, %q),\n", keys.Set(it.ID).String(), moduleName(it.Package))
	}
	w("];\n\n")

	w("pub static CURVES: &[(u32, &[f64])] = &[\n")
	for _, c := range slices.Sorted(maps.Keys(curves)) {
		var vs []string
		for _, v := range catalog.GrowCurve[c] {
			vs = append(vs, fl(v))
		}
		w("    (%d, &[%s]),\n", c, strings.Join(vs, ", "))
	}
	w("];\n")
	writeFile(path, b.String())
}

// moduleName is a package's module name under gcsimdata: its path below
// internal/<kind>/, with / as _.
func moduleName(pkg string) string {
	for _, root := range exportRoots {
		if rest, ok := strings.CutPrefix(pkg, root+"/"); ok {
			return strings.ReplaceAll(rest, "/", "_")
		}
	}
	return pkg
}

// rustName is SCREAMING_SNAKE_CASE for a Go identifier.
func rustName(s string) string {
	var b strings.Builder
	rs := []rune(s)
	for i, r := range rs {
		upper := r >= 'A' && r <= 'Z'
		if i > 0 && upper {
			prev := rs[i-1]
			nextLower := i+1 < len(rs) && rs[i+1] >= 'a' && rs[i+1] <= 'z'
			if (prev >= 'a' && prev <= 'z') || (prev >= '0' && prev <= '9') || (prev >= 'A' && prev <= 'Z' && nextLower) {
				b.WriteByte('_')
			}
		}
		b.WriteRune(r)
	}
	return strings.ToUpper(b.String())
}

const nActions = 15

// fitsI32 reports whether every int in v fits an i32.
func fitsI32(v any) bool {
	switch x := v.(type) {
	case json.Number:
		i, err := x.Int64()
		return err == nil && i >= -1<<31 && i < 1<<31
	case []any:
		for _, e := range x {
			if !fitsI32(e) {
				return false
			}
		}
	}
	return true
}

// frameTables reports whether v is a frame table (an int slice of gcsim's
// EndActionType entries) at the given depth, or nested ones of them.
func frameTables(t string, v any) bool {
	if t == "[int]" {
		xs, ok := v.([]any)
		return ok && len(xs) == nActions
	}
	inner, ok := strings.CutPrefix(t, "[")
	if !ok {
		return false
	}
	inner = strings.TrimSuffix(inner, "]")
	xs, ok := v.([]any)
	if !ok || len(xs) == 0 {
		return false
	}
	for _, x := range xs {
		if !frameTables(inner, x) {
			return false
		}
	}
	return true
}

// rustType is the Rust type of a gendata type; frames marks slices of
// EndActionType ints as FrameTable.
func rustType(t, ints string, frames bool) string {
	switch t {
	case "bool":
		return "bool"
	case "int":
		return ints
	case "f64":
		return "f64"
	case "str":
		return "&str"
	}
	if frames && t == "[int]" {
		return "FrameTable"
	}
	inner := strings.TrimSuffix(strings.TrimPrefix(t, "["), "]")
	return "&[" + rustType(inner, ints, frames) + "]"
}

func rustValue(t string, v any, frames bool) string {
	switch t {
	case "bool":
		return strconv.FormatBool(v.(bool))
	case "int":
		return string(v.(json.Number))
	case "f64":
		return fl(num(v.(json.Number)))
	case "str":
		return strconv.Quote(v.(string))
	}
	inner := strings.TrimSuffix(strings.TrimPrefix(t, "["), "]")
	var parts []string
	for _, e := range v.([]any) {
		parts = append(parts, rustValue(inner, e, frames))
	}
	if frames && t == "[int]" {
		return "[" + strings.Join(parts, ", ") + "]"
	}
	return "&[" + strings.Join(parts, ", ") + "]"
}

func writeGen(d *Dump, dir string) {
	check(os.RemoveAll(dir))
	var mods = map[string][]string{}
	for path, vals := range d.Packages {
		kind := ""
		for _, root := range exportRoots {
			if strings.HasPrefix(path, root+"/") {
				kind = strings.TrimPrefix(root, "internal/")
				if kind == "characters" {
					kind = "chars"
				}
			}
		}
		if kind == "" || len(vals) == 0 {
			continue
		}
		var b strings.Builder
		fmt.Fprintf(&b, "%s//! gcsim's %s.\n\n", header, path)
		body := len(b.String())
		used := map[string]bool{}
		any := false
		for _, name := range slices.Sorted(maps.Keys(vals)) {
			v := vals[name]
			rn := rustName(name)
			for used[rn] {
				rn += "_"
			}
			used[rn] = true
			ints := "i32"
			if !fitsI32(v.V) {
				ints = "i64"
			}
			frames := strings.Contains(strings.ToLower(name), "frames") && frameTables(v.T, v.V)
			typ := rustType(v.T, ints, frames)
			kw := "pub static"
			if !strings.HasPrefix(v.T, "[") {
				kw = "pub const"
			}
			if frames && v.T == "[int]" {
				kw = "pub static"
			}
			fmt.Fprintf(&b, "%s %s: %s = %s;\n", kw, rn, typ, rustValue(v.T, v.V, frames))
			any = true
		}
		if !any {
			continue
		}
		if text := b.String(); strings.Contains(text[body:], "FrameTable") {
			b.Reset()
			b.WriteString(text[:body] + "use crate::types::FrameTable;\n\n" + text[body:])
		}
		mod := moduleName(path)
		mods[kind] = append(mods[kind], mod)
		writeFile(filepath.Join(dir, kind, mod+".rs"), b.String())
	}
	var top strings.Builder
	top.WriteString(header + "//! Data generated from gcsim's packages: one module per character,\n//! weapon and artifact package, and the ICD tables.\n\n#![allow(dead_code, clippy::all)]\n\npub mod icd;\n")
	for _, kind := range slices.Sorted(maps.Keys(mods)) {
		fmt.Fprintf(&top, "pub mod %s;\n", kind)
		var m strings.Builder
		m.WriteString(header)
		for _, mod := range slices.Sorted(slices.Values(mods[kind])) {
			fmt.Fprintf(&m, "pub mod %s;\n", mod)
		}
		writeFile(filepath.Join(dir, kind, "mod.rs"), m.String())
	}
	writeFile(filepath.Join(dir, "mod.rs"), top.String())

	var b strings.Builder
	fmt.Fprintf(&b, "%s//! gcsim's ICD tags and groups (pkg/core/attacks), with each group's\n//! reset timer and its element application and damage sequences.\n\n", header)
	b.WriteString("#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub struct IcdTag(pub u16);\n\n#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub struct IcdGroup(pub u16);\n\n")
	for i, t := range d.ICDTags {
		if t != "" {
			fmt.Fprintf(&b, "pub const TAG_%s: IcdTag = IcdTag(%d);\n", rustName(t), i)
		}
	}
	b.WriteString("\n")
	for i, g := range d.ICDGroups {
		fmt.Fprintf(&b, "pub const GROUP_%s: IcdGroup = IcdGroup(%d);\n", rustName(g.Name), i)
	}
	b.WriteString("\n/// Reset timer, element application sequence and damage sequence, by group.\npub static GROUPS: &[(i32, &[f64], &[f64])] = &[\n")
	seq := func(xs []json.Number) string {
		var s []string
		for _, x := range xs {
			s = append(s, fl(num(x)))
		}
		return "&[" + strings.Join(s, ", ") + "]"
	}
	for _, g := range d.ICDGroups {
		fmt.Fprintf(&b, "    (%d, %s, %s), // %s\n", g.Reset, seq(g.Ele), seq(g.Dmg), g.Name)
	}
	b.WriteString("];\n")
	writeFile(filepath.Join(dir, "icd.rs"), b.String())
}
