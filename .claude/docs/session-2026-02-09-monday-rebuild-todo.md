# Monday Regression TODO (2026-02-09)

## Scope
This log captures Monday changes that should be treated as "candidate regressions" for manual re-introduction after restoring Sunday stability baseline.

- Sunday baseline commit: `edc45744` (2026-02-08 13:55:25 +0100)
- Monday range reviewed: `edc45744..db9a2d58`

## Monday commits in scope
1. `77c3bf30` - `.`
2. `5fade149` - `.`
3. `9d8e2251` - `.`
4. `820fae6c` - `entity: offload collision math to blocking thread and add micro-benchmark`
5. `dc1aa3b3` - `perf: skip collision math when no players online to reduce idle CPU load`
6. `3329824a` - `Create session-2026-02-09-changes.md`
7. `db9a2d58` - `.`

## High-risk items to rebuild manually
1. Collision pipeline changes (`Entity::adjust_movement_for_collisions`):
   - Avoid no-player semantic bypass in entity physics.
   - Keep behavior faithful; perf controls should live at server tick scheduler level.
2. Any scheduler/chunk-system edits touched around generation task orchestration:
   - Verify no `unwrap` assumptions on optional chunk holders.
   - Ensure cache extraction/restoration leaves holder state consistent.
3. Tick ordering for mobs:
   - AI goals + navigator + look control must run before living movement/physics.
4. Player first-join chunk hydration:
   - Chunk watch center must align with final spawn position before initial chunk subscription.
5. CPU-idle reduction intent:
   - Implement at ticker sleep cadence when player count is zero.
   - Keep wake-up latency low (interruptible sleep slices).

## Rebuild acceptance checklist
1. No panics in `GenerationSchedule::work` under first-join load.
2. No void/abyss first-join visual gap beyond expected chunk network latency.
3. Mob AI movement resumes normally (no frozen mobs due to loop ordering).
4. Idle server CPU reduced with zero online players.
5. `cargo fmt --all` and `RUSTFLAGS='-Dwarnings' cargo clippy -p pumpkin --all-targets --offline` pass.
