// Command hits runs one iteration of a gcsim config and prints every hit
// Arlecchino (or any character) lands and every Bond of Life change, for
// checking hits by hand (docs/arlecchino/report.md). It is a diagnostic:
// copy this directory to reference/hits (inside the reference module, which
// pins cenoda/gcsim@eb21884) and run
//
//	go run ./hits -config <file> [-seed 1] [-until 3600]
package main

import (
	"flag"
	"fmt"
	"os"

	"github.com/genshinsim/gcsim/pkg/core/attributes"
	"github.com/genshinsim/gcsim/pkg/core/event"
	"github.com/genshinsim/gcsim/pkg/core/info"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/simulation"

	_ "github.com/genshinsim/gcsim/pkg/simulator"
)

func main() {
	path := flag.String("config", "", "gcsim config")
	seed := flag.Int64("seed", 1, "seed")
	until := flag.Int("until", 1<<30, "last frame printed")
	flag.Parse()
	src, err := os.ReadFile(*path)
	if err != nil {
		panic(err)
	}
	file := ast.NewFile()
	cfg, gcsl, err := parser.New(file, string(src)).Parse()
	if err != nil {
		panic(err)
	}
	cfg.Settings.CollectStats = []string{"damage"}
	c, err := simulation.NewCore(*seed, false, cfg)
	if err != nil {
		panic(err)
	}
	ev, err := eval.NewEvaluator(file, gcsl, c)
	if err != nil {
		panic(err)
	}
	sim, err := simulation.New(cfg, ev, c)
	if err != nil {
		panic(err)
	}
	fmt.Println("frame\tchar\tabil\telem\tmult\tflat\tatk\tdmg%\tcr\tcd\tbol/maxhp\tcrit\treacted\tdamage")
	c.Events.Subscribe(event.OnEnemyDamage, func(args ...any) {
		if c.F > *until {
			return
		}
		atk := args[1].(*info.AttackEvent)
		ch := c.Player.ByIndex(atk.Info.ActorIndex)
		s := atk.Snapshot.Stats
		el := atk.Info.Element
		dmgp := s[attributes.DmgP] + s[attributes.EleToDmgP(el)]
		fmt.Printf("%d\t%s\t%s\t%s\t%.6f\t%.2f\t%.2f\t%.4f\t%.4f\t%.4f\t%.4f\t%v\t%v\t%.2f\n",
			c.F, ch.Base.Key, atk.Info.Abil, el, atk.Info.Mult, atk.Info.FlatDmg,
			ch.TotalAtk(), dmgp, s[attributes.CR], s[attributes.CD],
			ch.CurrentHPDebt()/ch.MaxHP(), args[3], atk.Info.Amped, args[2].(float64))
	}, "hits")
	c.Events.Subscribe(event.OnHPDebt, func(args ...any) {
		if c.F > *until {
			return
		}
		ch := c.Player.ByIndex(args[0].(int))
		fmt.Printf("%d\t%s\tBOND\t\t\t\t\t\t\t\t%.4f\t\t\t(change %.1f, max HP %.1f)\n", c.F, ch.Base.Key, ch.CurrentHPDebt()/ch.MaxHP(), -args[1].(float64), ch.MaxHP())
	}, "bond")
	if _, err := sim.Run(); err != nil {
		panic(err)
	}
}
