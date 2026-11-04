# Rotation search

> Status: Agreed scope and budget; proposed implementation, not started · Last updated: 2026-11-04

The owner commissioned the Rust port to make rotation search affordable. A rotation written for a C0 build can miss the best play at another constellation: Flins already changes at C1. Faster result pages are a benefit, but the next objective is to find better sustained-DPS rotations for the player's build, across the constellation stages below.

This document records the owner's decisions of 2026-11-04 and the proposed first implementation. It does not change engine code, releases, team approvals or ongoing porting work. Gscouter's `docs/cycle.md` records the fixed-rotation problem and the gains from existing alternatives; `internal/steady/steady.go` currently chooses rotations and fills waits rather than generating new action sequences.

## 1. Fixed scope

The first batch covers every library party, with each member independently at **C0, C1, C2, C4 or C6**. It is not limited to all-C0 and all-C6 parties, nor to five parties whose members all have the same constellation.

The owner's reason for excluding C3 and C5 is that they raise talent levels rather than introduce new abilities. They get no separate structural search: C3 uses the cycles found for C2, and C5 uses those for C4. Actual scoring still applies the player's real constellation and talent increases. This exclusion is already included in every cost estimate below; it buys no further reduction beyond `5^4` profiles per party.

The library snapshot inspected on 2026-11-04 (`Gscouter` checkout `e7d6d9a6`) contains 4,570 parties, including 4,332 marked supported and 238 draft. Thus the planning envelope is:

```
profiles per party = 5^4 = 625
all library profiles = 4,570 * 625 = 2,856,250
supported-party profiles = 4,332 * 625 = 2,707,500
```

The full count is a budget envelope, not a promotion of draft teams or a claim that every config already runs. Unsupported or invalid cases are reported explicitly. Freeze the library snapshot for a batch and record its commit.

### Standard equipment

Offline discovery uses one standard equipment configuration per party member, with these rules:

| Item | Rule |
|---|---|
| Weapon | Use the character's signature weapon when available; otherwise choose the weapon with the highest team-DPS ceiling for that role and party. |
| Refinement | Four-star weapons R5; five-star weapons R1. |
| Artifact set | The most common setup for that role; start from the party's library set and check discrepancies. |
| Main stats | The applicable KQM character-guide recommendation for that role. |
| Substats | A high-investment, physically attainable allocation, using the existing artifact scoring system; see below. |

The weapon rule selects **one weapon**, not a signature/non-signature branch for every member. The budget has no additional `2^4` weapon multiplier. Record the selected weapon and its source; do not silently call the library weapon the highest-ceiling alternative. Likewise, library frequency is a proxy to inspect, not measured player equipment frequency.

Use [KQM character guides](https://keqingmains.com/misc/artifacts/) for main-stat recommendations. The owner's higher-investment substat rule below replaces the ordinary KQM substat budget; do not import that budget merely because main stats come from KQM. Record the chosen guide, role and main stats. A character without an applicable guide needs an explicit documented choice before its build enters the batch.

### Substats

The agreed interpretation is approximately **40 CV per piece** for crit-oriented builds, with useful other substats in the remaining attainable allocation. When another substat is more valuable than crit, prioritize it even if CV falls. Do not force crit onto an EM, ER or other non-crit priority build.

Generate actual piece-level allocations: respect rarity, substat count, upgrade count, attainable values and the prohibition on repeating the main stat as a substat. Do not independently maximize CV and every other useful stat, or require an exactly 40 CV value if discrete rolls cannot produce it. Record the realized allocation, totals and artifact score.

Reuse Gscouter's `internal/artifact` roll values and scoring formula and `internal/weights` machinery. The existing weights are measured on the library build and its chosen rotation. They seed allocation; they are not a proof of the best allocation at a new weapon, constellation or cycle. Recheck priorities on representative profiles and final cycles within the reserve budget, particularly when ER changes burst frequency. Stop adding ER once another allocation improves team DPS more.

Freeze levels, base talent levels, character parameters and initial resources in the standard-build manifest before the full batch. Constellation talent bonuses are applied by the engine. Starting resources are conditions, not parameters the optimizer may inflate.

## 2. Objective and search representation

Optimize expected team DPS under the site's current looped conditions: 450 s duration, 150 s warmup, and the same enemy, energy and swap settings as Gscouter. Use an unchanged full fight for candidate scoring; do not substitute the first cycle's damage for sustained DPS.

Search a small rotation program, compiled to GCSL:

```
opening
repeat:
    ordered actions and blocks
    action parameters and repetition counts
    always / when-ready / every-N-cycle guards
    explicit waits and delays
```

A repeated block may contain different successive cycles, so a burst every second cycle is representable. The engine determines actual action timing, swaps, cancellation, energy waits, cooldowns and resource use. Preserve startup separately from the repeated section. An observed action list from one seed is only a seed candidate, not a translation of all the original script's conditions.

Insertion can use all supported actions of all four members, not only actions present in the C0 rotation. This allows the search to introduce a new on-field role. Bound program size, repeats and waits, and reject execution errors. A long wait is not by itself an invalid candidate; its effect belongs in the objective.

## 3. Algorithm: shared multi-start simulated annealing

The first implementation is a native Rust command, separate from the serving path. It runs multi-start simulated annealing and shares candidate programs across the profiles of a party. It does **not** run an independent full search for all 625 profiles.

### Search steps

1. Start from the library rotation, valid alternatives, large mutations of them and seeds with different on-field characters. Keep the current best existing result as a baseline candidate.
2. Start representative profiles with all-C0 and each of the four members individually changed to C1, C2, C4 and C6: 17 profiles. Allocate the other 15 of the 32 discovery slots to interacting constellations or profiles that the full-profile pass serves poorly. Discovery and screening can run in rounds.
3. Use eight logical annealing chains, scheduled on the available cores. The planning quota is 1,000 evaluated candidate programs per representative profile **across all its chains**, not per chain, with 16 fights per evaluation.
4. Share programs through a per-party archive. Preserve candidates that serve different profiles and roles, not just the largest absolute DPS. Export a shortlist of at most 16 programs, including the valid baseline, for the full-profile pass.
5. Score the shortlist on all 625 profiles. Reevaluate the top two on separate seeds. Feed low-performing or uncertain profiles back into the remaining discovery slots or the reserve budget.

The 32 slots are discovery locations, not an assertion that only 32 constellation combinations matter. All 625 combinations receive candidate comparisons. Additional archive comparisons and allocation checks must be charged to the quotas or reserve, not grow without accounting.

### Mutations and acceptance

Draw mutations that insert or delete an action, move an action or block, change supported action parameters, increase or decrease a repeat count, change a readiness/periodic guard, replace a combat block, or expand/contract the repeated sequence. Include occasional compound changes of three to six edits, so a useful change need not improve DPS after every single edit.

For current program `x`, proposed program `y`, fixed profile baseline DPS `D0` and temperature `T`:

```
delta = (DPS(y) - DPS(x)) / D0
accept if delta >= 0
otherwise accept with probability exp(delta / T)
```

Reject errored candidates. An idle or errored baseline cannot normalize the objective: first find a valid positive-DPS seed, or report that discovery failed. Calibrate initial temperature from sampled losses, for example to accept the median sampled loss with probability 0.5, then cool over the allocated evaluations. Keep the best discovered program separately from each chain's current program. Restart stagnant chains from a retained candidate with a large mutation. These schedules are initial implementation choices to calibrate, not measured optimal settings.

The method follows [simulated annealing](https://doi.org/10.1126/science.220.4598.671). Benchmark it against random mutations and improvement-only search with the same simulation budget. No global-optimum guarantee is claimed.

### Sampling

Use fixed search seeds for repeatable candidate comparisons, cache exact duplicate evaluations, and keep screening and reevaluation seeds separate. Different programs consume random numbers differently, so identical seed integers do not eliminate simulation noise.

The first budget uses 16 fights for discovery, 8 for full-profile screening and 128 fresh fights for each of the two shortlisted programs. Record uncertainty and use the reserve for ambiguous or unstable winners. The 128-fight stage is reranking, not a declaration that every candidate satisfies the engine's gcsim parity criterion. Check new mechanics reached by winning programs against gcsim selectively within the remaining budget; existing engine accuracy and approval policies remain separate.

## 4. One week and $500

The owner's hard resource envelope is **one week, $500 total, and two 64-core Fals v7 VMs**, with Spot permitted. Do not extend the deadline or add machines to compensate for overruns.

[Fals v7](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/compute-optimized/falsv7-series) maps each vCPU to a physical core. The nominal weekly capacity is:

```
128 cores * 168 h = 21,504 core-hours
planning allowance at 70% effective utilization = 15,052.8 CPU-hours
```

The 70% allowance covers interruptions, scheduling and other overhead for planning. It is an assumption, not a measured availability promise. The VM rate is billed for allocated time, not only useful simulation time.

### Throughput basis and work allocation

Measured locally on 2026-11-04 using the existing release binary, not rebuilt for this timing:

```
engine/target/release/gse configs/team-looped.txt 500 150 1
```

Two 500-fight runs used 5.986 and 5.958 CPU-seconds. The estimate rounds to **12 ms per 450 s fight**. This is one config on the session machine, not an average over the library or a Fals v7 measurement. Benchmark representative parties, action-heavy programs and energy-starved profiles on the actual VMs before setting final quotas.

| Stage | Fight count | CPU-hours at 12 ms/fight |
|---|---:|---:|
| Shared discovery: 4,570 parties * 32 profiles * 1,000 candidates * 16 fights | 2,339,840,000 | 7,799.47 |
| All-profile screening: 2,856,250 profiles * 16 candidates * 8 fights | 365,600,000 | 1,218.67 |
| Fresh-seed reranking: 2,856,250 profiles * 2 candidates * 128 fights | 731,200,000 | 2,437.33 |
| Main work | 3,436,640,000 | **11,455.47** |
| Reserve within the 70% allowance | | **3,597.33** |

At the assumed throughput and utilization, main work takes about **5.33 days** on 128 cores. Standard-build preparation, stat checks, initialization, extra comparisons, selective gcsim checks, retries and packaging consume the reserve. This does not prove the algorithm converges in 1,000 candidates.

The reduction from seven constellation stages is already `625 / 2401`, about 26% of the earlier profile count. There is no second C3/C5 discount to apply. Independent 38,144-fight searches for every reduced profile would still cost about 363,163 CPU-hours; shared discovery is essential.

### Cost controls and Spot recovery

An Azure Retail Prices API lookup during planning returned Linux `F64als v7 Spot` in `westus2` at $0.715546 per VM-hour. At an unchanged rate, two VMs for 168 h cost about $240.42 in compute. This is a dated planning observation, not a reservation or capacity guarantee; [query the API](https://prices.azure.com/api/retail/prices?$filter=armSkuName%20eq%20%27Standard_F64als_v7%27%20and%20priceType%20eq%20%27Consumption%27) again at execution.

The proposed per-VM price ceiling is $1.30/h: two VMs for the full week have a compute ceiling of $436.80, leaving $63.20 of the $500 envelope for storage and other charges. Account for the actual applicable charges before allocation. Track accumulated and projected total spend and enforce a stop with margin; a cloud budget alert alone is not the worker's stop mechanism.

[Azure Spot](https://learn.microsoft.com/en-us/azure/virtual-machines/spot-vms) can evict for capacity as well as price, and advance notice is best effort. Persist completed work and checkpoints periodically, independently of eviction notification. Include program, profile, RNG state, seeds, sample counts, temperature, archive and consumed budget. Resume idempotent work units from external storage without rerunning completed slices. Keep worker parallelism within 64 single-core workers per VM; do not multiply that by an inner 64-thread simulation pool.

Use the first pilot to forecast remaining CPU time from the measured workload mix. Adjust discovery depth and uncertain-result allocations within the hard deadline and spending cap. If even minimum full-profile work no longer fits, report incomplete profiles and their reason rather than quietly dropping them or claiming completion. Outstanding porting work is an explicit dependency, not a reason to change other agents' branches.

## 5. Using results on real builds

Standard equipment makes offline discovery finite. It does not replace the player's equipment or stats when scoring. Retain candidate GCSL programs and their applicability records; on a real build, evaluate candidate cycles using the actual constellation, weapon, refinement, artifact set, stats and talents. A different set is evaluated as equipped, with no assumed missing set effect.

Reuse the stored cycles and existing filler path first; any additional local search has a separate bounded serving budget and needs measurement. C3 and C5 reuse their preceding structural stage as described in section 1, with their actual talent increases. Do not interpolate DPS or automatically expose the offline standard-build score as the player's result.

No serving flag, team approval or published library rotation changes as part of documenting this plan. Engine portability, stochastic search quality and approval to move a team to the engine remain separate questions.

## 6. Deliverables and first implementation order

1. A versioned manifest of standard builds, sources, per-piece allocations and the frozen library and engine commits.
2. The bounded native search command, full-profile comparison runner, candidate cache and resumable worker queue with spend and time accounting.
3. A pilot including Flins C0/C1 and interacting-constellation parties. Measure throughput, search improvement and transfer to different actual builds; compare algorithms at equal cost, including trials where the known good alternative is withheld from starting seeds.
4. Within the one-week envelope, candidate comparisons for all eligible 625 profiles per party, a reusable candidate archive and a report listing completed, unsupported, failed, uncertain and deferred profiles. Each result includes its program, seeds, sample count, DPS, baseline difference, uncertainty and consumed CPU time.
5. An independently reevaluated set of improvements, with selective gcsim checks of newly reached behavior. Record limitations; finite-budget search finds the best evaluated program, not a proven optimum.

The document push is authorized directly to `main` with CI skipped by the owner. It carries documentation only; implementation and batch execution are subsequent work.
