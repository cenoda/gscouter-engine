# Work in progress

`klee.patch` (`git apply wip/klee.patch`): Klee at C0, with gcsim's
`action.Info.QueueAction` for the engine (a task that runs once an action
has lasted some frames, and never if another action replaces it first).
Eight of her nine library teams pass; Klee / Fischl / Xingqiu / Bennett
is -1.78% at 1,000 iterations, every ability about 2% fewer hits. What
was checked and agrees with gcsim (200 seeds each): energy by source,
Xingqiu's particles per skill and on-field share, A1 sparks and charged
attacks in the first cycle, every action-to-action gap, the cycle length
(1271.6 frames). What differs: Xingqiu's and Bennett's energy waits
(19.5 s against 23.1 s for Xingqiu), the swap-to-Xingqiu-skill wait
(54.0 against 56.4 frames), and how often the first cycle ends on a walk
(26 against 40 of 200). With their ER raised the team agrees within
0.2%. The patch's regression scores are unchanged.
