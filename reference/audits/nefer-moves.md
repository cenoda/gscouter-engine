# Nefer moved-player audit

Reference: `cenoda/gcsim@eb218847f82b`, fixed by `reference/go.mod`. Nefer's held charged attack (Slither) moves the player toward the enemy, 0.1 a frame (gcsim's `SetDirection` to the target, then `SetPlayerPos` at `CalcOffsetPoint(pos, (0, 0.1), dir)`), so every member of a team that holds her charged attack needs `MOVES_WITH_PLAYER`. Her 16 library teams add Lauma (all 16), Linnea (2), Kokomi (1), Collei (1) and Zhongli (1) to the members already in it; Collei and Zhongli are on teams that also need Aino and are left out, so they stay errors there (`gse --check`: `unsupported: nefer charge[hold] (Slither moves the player) with <name>`).

## What the packages read

Read in full: gcsim's `internal/characters/{nefer,lauma,linnea,kokomi}`.

- **Nefer**: normal attacks and the ordinary charged attack are `NewBoxHit(player, target, offset, w, h)` built at the action (the charged attack's at its release, after Slither); the skill `NewCircleHit(player, target, (0, 1), 3)` at the action; the burst `NewBoxHit(player, target, nil, 6, 10)` at the action, both hits; Phantasm Performance one `NewCircleHitOnTarget(player, nil, 5)` built when the Performance starts and used by every hit; the seed absorb `GadgetsWithinArea(NewCircleHitOnTarget(player, nil, 6))` (seed centers inside) at each Slither step, at the Performance's Dew frame and at the charged attack's hitmark; C4's shred by the player's distance to the enemy, `> 10 + radius`, every frame.
- **Lauma**: normal and charged attacks `NewBoxHit(player, target, nil, ...)` at the action; the pressed and held skill and C6's hits `NewCircleHit(player, player.Pos(), nil, 6)` and Frostgrove Sanctuary's ticks and the held skill's Lunar-Bloom `NewCircleHitOnTarget(player, nil, 6)`, each built when it is queued (the ticks at each tick). Spirit Envoy, the burst and C1's heal read no position.
- **Kokomi**: normal attacks, C1 and the charged attack are circles on the enemy; the burst `NewCircleHitOnTarget(player, nil, 5)` at the action; the jellyfish `NewCircleHitOnTarget(player, (0, 3), 6)` built at the skill and kept for every tick, and each tick heals when the player's center is inside it.
- **Linnea**: normal and aimed shots `NewBoxHit(player, target, (0, -0.5), 0.1, 1)` at the action; everything else is on Lumi, which gcsim puts on the enemy (`LumiPos`).

The engine's modules build the same shapes at the same moments; no code changed for Lauma, Linnea or Kokomi beyond adding them to `MOVES_WITH_PLAYER`.

## Fixtures

Each case ran 1,000 iterations, seeds 1–1000, warmup 0, four workers (`nefer-moves-audit.py`). The target is at (0,0), radius 1. The fixtures are deterministic: every crit is certain (Kokomi at CR 200% for her −100%), every member has 1,000% ER so energy never decides a burst, Lauma is ascendant (Nefer is on her fixture) so blooms never crit, and Kokomi is C5 so C6's HP check does not read the bloom self damage. The positions keep every Seed of Deceit (dendro cores appear 1.5 from the enemy at a random angle) wholly inside or outside the 6 of the absorb.

| Fixture | Position | gcsim team DPS | Engine team DPS | What it checks |
| --- | --- | ---: | ---: | --- |
| nefer-moves | 0,-1.5 | 35222.7983 | 35222.7983 | near; Phantasm, absorb, Slither, burst |
| nefer-moves | 0,-3 | 35222.7983 | 35222.7983 | |
| nefer-moves | 2.4,3.2 | 35222.7983 | 35222.7983 | rotated facing |
| nefer-moves | 0,-25 | 3898.6768 | 3898.6768 | far: no bloom, no Dew, five ordinary charged attacks, one after 60 frames of Slither |
| nefer-moves-c4 | 0,-11 | 3592.8357 | 3592.8357 | C4's shred touches at 10 + 1 |
| nefer-moves-c4 | 0,-11.0001 | 3238.1810 | 3238.1810 | and misses beyond |
| lauma-kokomi-moves | 0,-1.5 | 44246.9366 | 44246.9366 | near |
| lauma-kokomi-moves | 0,-5.9 | 44246.9366 | 44246.9366 | |
| lauma-kokomi-moves | 0,-6 | 44246.9366 | 44246.9366 | Kokomi's burst touches at 5 + 1 |
| lauma-kokomi-moves | 0,-6.0001 | 43999.2392 | 43999.2392 | and misses |
| lauma-kokomi-moves | 0,-7 | 43999.2392 | 43999.2392 | Lauma's circles touch at 6 + 1 |
| lauma-kokomi-moves | 0,-7.0001 | 9429.0963 | 9429.0963 | and miss |
| lauma-kokomi-moves | 0,-10 | 9429.0963 | 9429.0963 | the jellyfish (3 ahead, radius 6) touches |
| lauma-kokomi-moves | 0,-10.0001 | 8463.4088 | 8463.4088 | and misses |
| lauma-kokomi-moves | 3,4 | 44246.9366 | 44246.9366 | rotated facing (the jellyfish's offset) |
| lauma-kokomi-moves | 0,-15 | 8463.4088 | 8463.4088 | far |
| lauma-kokomi-moves-timing | moves | 18253.7841 | 18253.7841 | the jellyfish keeps its cast place, the sanctuary's ticks follow each move |
| linnea-moves | 0,-1.5 | 10188.0375 | 10188.0375 | near |
| linnea-moves | 3,4 | 10188.0375 | 10188.0375 | rotated facing |
| linnea-moves | 0,-15 | 10105.0921 | 10105.0921 | |
| linnea-moves | 0,-25 | 7116.3518 | 7116.3518 | Moondrifts beyond 20 of the player |

Every ability's hits and damage agree in every case (`nefer-moves-results.txt`, 0 differences), and so do the waits.

## Reproduction

From `reference/audits`, with the release binary built:

```sh
python3 nefer-moves-audit.py nefer-moves.txt 0,-1.5 0,-3 2.4,3.2 0,-25
python3 nefer-moves-audit.py nefer-moves-c4.txt 0,-11 0,-11.0001
python3 nefer-moves-audit.py lauma-kokomi-moves.txt 0,-1.5 0,-5.9 0,-6 0,-6.0001 0,-7 0,-7.0001 0,-10 0,-10.0001 3,4 0,-15
python3 nefer-moves-audit.py lauma-kokomi-moves-timing.txt
python3 nefer-moves-audit.py linnea-moves.txt 0,-1.5 3,4 0,-15 0,-25
```

These are local geometry checks; the library teams and probes are compared by CI.
