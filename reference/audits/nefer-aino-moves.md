# Aino, Collei and Zhongli moved-player audit

Reference: `cenoda/gcsim@eb218847f82b`, fixed by `reference/go.mod`. The six `nefer-aino-*` library teams hold Nefer's charged attack, whose Slither moves the player, so every member needs `MOVES_WITH_PLAYER` (`nefer-moves.md`): Aino on all six, and Collei and Zhongli on the two that carry them.

## What the packages read

Read in full: gcsim's `internal/characters/{aino,collei,zhongli}`.

- **Aino**: normal attacks circles and a box on the player at the action; the skill's first hit `NewCircleHitOnTarget(PrimaryTarget(), (0, off), r)` and its second on the player, both built at the action; the burst's area `NewCircleHitOnTarget(player, nil, 10)` built at the cast and kept, each ball at its tick on a random enemy within that area that no ball marked in the last 0.8 s, else on `CalcRandomPointFromCenter(area center, 1.5, 9)`; C-hits on the enemy.
- **Collei**: normal and aimed shots `NewBoxHit(player, target, …)` at the action; the skill's two hits `NewCircleHitOnTarget(player, nil, 2)` built at the action; the A1 sprout's ticks the same circle built at each tick; the burst `burstPos` = the player's position at the cast, the explosion (5.5) and leaps (4) on it, A4's zone a circle of 6 on it that the player must be inside at each 30-frame check (the checks stop for good once the player is outside); C6 on the enemy.
- **Zhongli**: normal attacks fans and boxes on the player at the action; the charged attack on the enemy; the burst `NewCircleHitOnTarget(player, (0, 5), 7.5 or 9)` at the action; the stele placed 3 ahead of the player along its direction (initial hit a circle of 2 there), each tick `NewBoxHitOnTarget(stelePos + (0, -4) along the stele's direction, nil, 8, 8)` built at the tick from that stele, the box on a point and so facing the default direction, and the resonance boxes the same from each construct within 8; the Jade Shield's shred on `EnemiesWithinArea(NewCircleHitOnTarget(player, nil, 7.5))` (the enemy's center inside) at each of its 18-frame tasks.

Aino's and Collei's modules built the same shapes at the same moments. Zhongli's did not in two places, both moot with the player at the origin facing the enemy (every library build; the two scores are unchanged), and fixed here:

- a stele's own tick reused the newest stele's box, built from the player and turned with the player's facing; it is now built at each tick from that stele, facing the default direction;
- the Jade Shield's shred reached the enemy wherever the player stood; it now needs the enemy's center within 7.5 of the player at each task.

## Fixtures

Each case ran 1,000 iterations, seeds 1–1000, warmup 0, four workers (`moves-audit.py`, Nefer's script). The target is at (0,0), radius 1; every crit is certain and every member has 1,000% ER.

| Fixture | Position | gcsim team DPS | Engine team DPS | What it checks |
| --- | --- | ---: | ---: | --- |
| zhongli-moves | 0,-1.5 | 4025.1543 | 4025.1543 | near |
| zhongli-moves | 0,-4 | 3822.5846 | 3822.5846 | the stele tick's box touches at 4 |
| zhongli-moves | 0,-4.0001 | 3822.5846 | 3822.5846 | |
| zhongli-moves | 0,-6 | 3611.6698 | 3611.6698 | the stele's initial circle touches at 3 + 2 + 1 |
| zhongli-moves | 0,-6.0001 | 3507.8217 | 3507.8217 | and misses |
| zhongli-moves | 0,-7.5 | 3507.8217 | 3507.8217 | the shield's shred reaches at 7.5 |
| zhongli-moves | 0,-7.5001 | 3048.6404 | 3048.6404 | and not beyond |
| zhongli-moves | 0,-13.5 | 1332.2287 | 1332.2287 | the burst touches at 5 + 7.5 + 1 |
| zhongli-moves | 0,-13.5001 | 111.8465 | 111.8465 | and misses |
| zhongli-moves | 3,4 | 1801.8922 | 1801.8922 | rotated facing (the stele's direction and its tick box) |
| zhongli-moves | 0,-20 | 111.8465 | 111.8465 | far |
| zhongli-moves-timing | moves | 2414.5815 | 2414.5815 | two steles from different places tick where each stands; the shred follows the player |
| collei-aino-moves | 0,-1.5 | 3663.5512 | 3663.5512 | near; blooms and dendro cores |
| collei-aino-moves | 0,-3 | 3663.5512 | 3663.5512 | Collei's skill and sprout circles touch at 2 + 1 |
| collei-aino-moves | 0,-3.0001 | 2927.2375 | 2927.2375 | and miss |
| collei-aino-moves | 0,-5 | 2766.5149 | 2766.5149 | the leaps touch at 4 + 1 |
| collei-aino-moves | 0,-5.0001 | 1563.8982 | 1563.8982 | and miss |
| collei-aino-moves | 0,-6.5 | 1226.4401 | 1226.4401 | the explosion touches at 5.5 + 1 |
| collei-aino-moves | 0,-6.5001 | 812.4938 | 812.4938 | and misses |
| collei-aino-moves | 0,-10 | 812.4938 | 812.4938 | Aino's balls take the enemy within 10 of the cast |
| collei-aino-moves | 0,-10.0001 | 550.6221 | 551.4894 | random points instead (see below) |
| collei-aino-moves | 3,4 | 2766.5149 | 2766.5149 | rotated facing |
| collei-aino-moves | 0,-20 | 546.4465 | 546.4465 | far |
| collei-aino-moves-timing | moves | 2216.3105 | 2216.3105 | the burst areas stay where cast; the sprout and A4's zone follow the player |

Every ability's hits and damage agree in every case (`nefer-aino-moves-results.txt`), and so do the waits, but one: at 0,-10.0001 Aino's balls fall on random points 1.5 to 9 from where she cast the burst, some of which reach the enemy, and gcsim and the engine draw those points from their own generators, so fights differ one by one (the formula is the same, `CalcRandomPointFromCenter` against `Sim::random_point`). At 20,000 iterations they agree: the balls hit 0.0916 against 0.0923 times a fight, and team DPS is 551.1165 ± 0.1256 against 551.1276 ± 0.1258.

## Reproduction

From `reference/audits`, with the release binary built:

```sh
python3 moves-audit.py zhongli-moves.txt 0,-1.5 0,-4 0,-4.0001 0,-6 0,-6.0001 0,-7.5 0,-7.5001 0,-13.5 0,-13.5001 3,4 0,-20
python3 moves-audit.py zhongli-moves-timing.txt
python3 moves-audit.py collei-aino-moves.txt 0,-1.5 0,-3 0,-3.0001 0,-5 0,-5.0001 0,-6.5 0,-6.5001 0,-10 0,-10.0001 3,4 0,-20
python3 moves-audit.py collei-aino-moves-timing.txt
```

These are local geometry checks; the library teams and probes are compared by CI.
