// Command dump prints each character's base stats after gcsim set them up
// (character and weapon curves, ascension stats and the config's added
// stats), which the Rust engine takes as data instead of porting the
// curves.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/genshinsim/gcsim/pkg/core/attributes"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/simulation"

	_ "github.com/genshinsim/gcsim/pkg/simulator"
)

type char struct {
	Name      string             `json:"name"`
	Level     int                `json:"level"`
	Cons      int                `json:"cons"`
	Ascension int                `json:"ascension"`
	Talents   [3]int             `json:"talents"`
	Element   string             `json:"element"`
	Weapon    string             `json:"weapon"`
	Refine    int                `json:"refine"`
	Stats     map[string]float64 `json:"stats"`
}

func main() {
	raw, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	file := ast.NewFile()
	cfg, gcsl, err := parser.New(file, string(raw)).Parse()
	if err != nil {
		panic(err)
	}
	c, err := simulation.NewCore(1, false, cfg)
	if err != nil {
		panic(err)
	}
	ev, err := eval.NewEvaluator(file, gcsl, c)
	if err != nil {
		panic(err)
	}
	if _, err := simulation.New(cfg, ev, c); err != nil {
		panic(err)
	}
	var out []char
	for _, ch := range c.Player.Chars() {
		s := map[string]float64{}
		for i, v := range ch.BaseStats {
			if v != 0 {
				s[attributes.StatTypeString[i]] = v
			}
		}
		out = append(out, char{
			Name: ch.Base.Key.String(), Level: ch.Base.Level, Cons: ch.Base.Cons, Ascension: ch.Base.Ascension,
			Talents: [3]int{ch.Talents.Attack, ch.Talents.Skill, ch.Talents.Burst},
			Element: ch.Base.Element.String(), Weapon: ch.Weapon.Key.String(), Refine: ch.Weapon.Refine, Stats: s,
		})
	}
	b, _ := json.MarshalIndent(out, "", "  ")
	fmt.Println(string(b))
}
