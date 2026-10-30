//go:build gendata

// Command zzgendata runs inside a temporary copy of gcsim that gendata
// prepares, never in gcsim itself. gendata adds to every character, weapon
// and artifact package (and pkg/core) a file registering the package's
// variables and constants with pkg/zzgendata, so their values are read as
// gcsim computes them, including frame tables its init functions build.
// It prints them as JSON, with what gcsim's characters set up at runtime
// (energy, normal hit count, constellations, body, animation delays) and
// the ICD tables.
package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"reflect"
	"runtime"
	"slices"
	"strings"

	"github.com/genshinsim/gcsim/pkg/catalog"
	"github.com/genshinsim/gcsim/pkg/core"
	"github.com/genshinsim/gcsim/pkg/core/attacks"
	"github.com/genshinsim/gcsim/pkg/core/info"
	"github.com/genshinsim/gcsim/pkg/core/keys"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/simulation"
	"github.com/genshinsim/gcsim/pkg/zzgendata"

	_ "github.com/genshinsim/gcsim/pkg/simulator"
)

const module = "github.com/genshinsim/gcsim/"

// Val is a value with its type: bool, int, f64, str, or [T] for a slice.
type Val struct {
	T string `json:"t"`
	V any    `json:"v"`
}

type Char struct {
	Key          string  `json:"key"`
	ID           int     `json:"id"`
	Package      string  `json:"package"`
	Error        string  `json:"error,omitempty"`
	Element      int     `json:"element"`
	WeaponClass  int     `json:"weapon_class"`
	Body         int     `json:"body"`
	Zone         int     `json:"zone"`
	Rarity       int     `json:"rarity"`
	EnergyMax    float64 `json:"energy_max"`
	NormalHitNum int     `json:"normal_hit_num"`
	NormalCon    int     `json:"normal_con"`
	SkillCon     int     `json:"skill_con"`
	BurstCon     int     `json:"burst_con"`
	// AnimDelay is AnimationStartDelay by gcsim's AnimationDelayKey.
	AnimDelay []int `json:"anim_delay"`
}

type Item struct {
	Key     string `json:"key"`
	ID      int    `json:"id"`
	Package string `json:"package"`
}

type ICDGroup struct {
	Name  string    `json:"name"`
	Reset int       `json:"reset"`
	Ele   []float64 `json:"ele"`
	Dmg   []float64 `json:"dmg"`
}

type Out struct {
	Packages  map[string]map[string]Val `json:"packages"`
	Chars     []Char                    `json:"chars"`
	Weapons   []Item                    `json:"weapons"`
	Sets      []Item                    `json:"sets"`
	ICDGroups []ICDGroup                `json:"icd_groups"`
	ICDTags   []string                  `json:"icd_tags"`
}

func typeOf(t reflect.Type) (string, bool) {
	switch t.Kind() {
	case reflect.Bool:
		return "bool", true
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64,
		reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return "int", true
	case reflect.Float32, reflect.Float64:
		return "f64", true
	case reflect.String:
		return "str", true
	case reflect.Slice, reflect.Array:
		e, ok := typeOf(t.Elem())
		return "[" + e + "]", ok
	}
	return "", false
}

func value(v reflect.Value) (any, bool) {
	switch v.Kind() {
	case reflect.Bool:
		return v.Bool(), true
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return v.Int(), true
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return int64(v.Uint()), true
	case reflect.Float32, reflect.Float64:
		f := v.Float()
		return f, !math.IsNaN(f) && !math.IsInf(f, 0)
	case reflect.String:
		return v.String(), true
	case reflect.Slice, reflect.Array:
		out := make([]any, v.Len())
		for i := range out {
			x, ok := value(v.Index(i))
			if !ok {
				return nil, false
			}
			out[i] = x
		}
		return out, true
	}
	return nil, false
}

// pkgOf is the package path, under gcsim, of a registered function.
func pkgOf(f any) string {
	name := runtime.FuncForPC(reflect.ValueOf(f).Pointer()).Name()
	name = strings.TrimPrefix(name, module)
	if i := strings.LastIndex(name, "."); i >= 0 {
		name = name[:i]
	}
	return name
}

var starter = []string{"dullblade", "wastergreatsword", "beginnersprotector", "huntersbow", "apprenticesnotes"}

// setup is what a character's NewChar and Init set, on a lone level 90
// character with a 1-star weapon of its class.
func setup(k keys.Char) (c Char) {
	c = Char{Key: k.String(), ID: int(k)}
	defer func() {
		if r := recover(); r != nil {
			c.Error = fmt.Sprint(r)
		}
	}()
	var last error
	for _, w := range starter {
		cfg := fmt.Sprintf(`%[1]s char lvl=90/90 cons=0 talent=9,9,9;
%[1]s add weapon="%[2]s" refine=1 lvl=90/90;
options iteration=1;
target lvl=100 resist=0.1;
active %[1]s;
%[1]s attack;
`, k.String(), w)
		file := ast.NewFile()
		simcfg, gcsl, err := parser.New(file, cfg).Parse()
		if err == nil && len(simcfg.Errors) > 0 {
			err = simcfg.Errors[0]
		}
		if err != nil {
			last = err
			continue
		}
		core, err := simulation.NewCore(1, false, simcfg)
		if err != nil {
			last = err
			continue
		}
		ev, err := eval.NewEvaluator(file, gcsl, core)
		if err != nil {
			last = err
			continue
		}
		if _, err := simulation.New(simcfg, ev, core); err != nil {
			last = err
			continue
		}
		ch := core.Player.Chars()[0]
		c.Element = int(ch.Base.Element)
		c.WeaponClass = int(ch.Weapon.Class)
		c.Body = int(ch.CharBody)
		c.Zone = int(ch.CharZone)
		c.Rarity = ch.Base.Rarity
		c.EnergyMax = ch.EnergyMax
		c.NormalHitNum = ch.NormalHitNum
		c.NormalCon, c.SkillCon, c.BurstCon = ch.NormalCon, ch.SkillCon, ch.BurstCon
		c.AnimDelay = []int{0, ch.AnimationStartDelay(info.AnimationYelanN0StartDelay), ch.AnimationStartDelay(info.AnimationXingqiuN0StartDelay)}
		return c
	}
	c.Error = fmt.Sprint(last)
	return c
}

func main() {
	out := Out{Packages: map[string]map[string]Val{}}
	for path, f := range zzgendata.Packages {
		vals := map[string]Val{}
		for name, v := range f() {
			rv := reflect.ValueOf(v)
			if !rv.IsValid() {
				continue
			}
			t, ok := typeOf(rv.Type())
			if !ok {
				continue
			}
			x, ok := value(rv)
			if !ok {
				continue
			}
			vals[name] = Val{T: t, V: x}
		}
		out.Packages[path] = vals
	}

	charKeys := slices.Collect(func(yield func(keys.Char) bool) {
		for k := range core.NewCharFuncMap {
			if !yield(k) {
				return
			}
		}
	})
	slices.Sort(charKeys)
	for _, k := range charKeys {
		c := setup(k)
		c.Package = pkgOf(core.NewCharFuncMap[k])
		if d, ok := catalog.CharacterMap[k]; ok && c.Rarity == 0 {
			c.Rarity = int(d.Rarity)
		}
		out.Chars = append(out.Chars, c)
	}

	coreVars := zzgendata.Packages["pkg/core"]()
	items := func(m any) []Item {
		var res []Item
		rv := reflect.ValueOf(m)
		for _, k := range rv.MapKeys() {
			res = append(res, Item{Key: fmt.Sprint(k.Interface()), ID: int(k.Int()), Package: pkgOf(rv.MapIndex(k).Interface())})
		}
		slices.SortFunc(res, func(a, b Item) int { return a.ID - b.ID })
		return res
	}
	out.Weapons = items(coreVars["weaponMap"])
	out.Sets = items(coreVars["setMap"])

	attacksVars := zzgendata.Packages["pkg/core/attacks"]()
	for i := range attacks.ICDGroupResetTimer {
		out.ICDGroups = append(out.ICDGroups, ICDGroup{
			Reset: attacks.ICDGroupResetTimer[i],
			Ele:   attacks.ICDGroupEleApplicationSequence[i],
			Dmg:   attacks.ICDGroupDamageSequence[i],
		})
	}
	out.ICDTags = make([]string, 0)
	for name, v := range attacksVars {
		switch x := v.(type) {
		case attacks.ICDGroup:
			if int(x) < len(out.ICDGroups) {
				out.ICDGroups[x].Name = strings.TrimPrefix(name, "ICDGroup")
			}
		case attacks.ICDTag:
			for len(out.ICDTags) <= int(x) {
				out.ICDTags = append(out.ICDTags, "")
			}
			out.ICDTags[x] = strings.TrimPrefix(name, "ICDTag")
		}
	}

	enc := json.NewEncoder(os.Stdout)
	if err := enc.Encode(out); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
