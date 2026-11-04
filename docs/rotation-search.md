# Rotation search

> Status: Moved to gscouter-optimizer · Last updated: 2026-11-04

The authoritative [rotation-search plan](https://github.com/cenoda/gscouter-optimizer/blob/main/docs/rotation-search.md) has moved to [gscouter-optimizer](https://github.com/cenoda/gscouter-optimizer). That repository owns standard-build preparation, rotation search, batch scheduling, checkpoints and compute budgets; it currently contains documentation only.

gscouter-engine owns the simulator and engine verification. Gscouter owns site integration and scoring candidates on real user builds. The optimizer is planned to use the engine library at a pinned commit and exchange versioned data and GCSL artifacts with the site. This move changes no engine code, releases, approvals or ongoing porting work.

The original plan remains available in [commit 84a53e4](https://github.com/cenoda/gscouter-engine/blob/84a53e45e91a1c9ce3cabaaf2cf2c7082283bc53/docs/rotation-search.md).
