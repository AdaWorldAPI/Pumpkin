# Session 016 — Entity: Upstream Sync + Vanilla Parity Fixes

**Agent:** entity
**Date:** 2026-02-20
**Branch:** claude/entity-spawning-ai-V7oqj

## Preamble

- Read upstream sync analysis at `.claude/docs/upstream-sync-2026-02-20.md`
- Read Session 015 log (passive mob breeding goals)
- Read entity decisions ENT-001 through ENT-018
- Rebased onto origin/master (d46900a) — clean, no conflicts

## What I Did

### 1. Cherry-Picked 5 Clean Upstream Fixes (Zero Conflict)
All 5 applied cleanly as identified in the sync analysis:
- `8433f05` — Prevent mobs from attacking when already dead (mob/mod.rs)
- `3f170e4` — Entity really teleport instead of only packet (entity/mod.rs)
- `a653726` — Preserve knockback velocity on killing blow (living.rs)
- `528d5b3` — Check every hitbox on block placement (net/java/play.rs) *[required minor fix: CollisionShape→BoundingBox conversion]*
- `ba40fcd` — playersSleepingPercentage gamerule support (world/mod.rs)

### 2. Ported TNT Fuse Underflow Fix (from upstream 59b7e0a)
- Bug: `fetch_sub(1)` on u32 fuse returns value before subtraction. When fuse==0, it becomes u32::MAX (underflow), preventing explosion.
- Fix: Use `load()` first, check `<= 1`, then `store(fuse - 1)`.
- The spectator immunity part of the same commit requires entity damage in explosions (not yet implemented), so skipped.

### 3. Audited 14 Upstream AI Commits vs Our 81-Mob System
Produced detailed comparison report. Key findings:

**New goals upstream has that we DON'T have (5):**
- EatGrassGoal (sheep-specific)
- RevengeGoal (mob retaliates against last attacker)
- BegGoal (wolf begging animation)
- OwnerHurtByTargetGoal (tamed mob defends owner)
- OwnerHurtTargetGoal (tamed mob attacks owner's target)

**Parameter differences found and fixed (3):**
- FollowParentGoal: search_range 16→8 (vanilla), stop_distance_sq 81→9 (vanilla)
- PanicGoal: flee range -10..10 → -5..5 (vanilla)
- CreeperIgniteGoal: stop() was no-op TODO → now resets fuse_speed to -1

**Commits safe to skip (4):** 05763ea, 68f90e5, 8bd5009, 392d181 (our 81-mob system already covers them)

**Multi-conflict upstream ports deferred (need cross-agent coordination):**
- Collision fix (e438e09) — needs pumpkin-util AtomicF32 + block/ changes
- Totem fix (5a5b9e4) — needs pumpkin-world + block/ changes
- Item drop velocity (15fbf8f) — needs block/ changes
- Dynamic eye height (74ecc3b) — 3 conflicts in command/player/projectile

### 4. Added Entity::get_eye_height() Method
Ported from upstream `74ecc3b`. Returns pose-aware eye height (sneaking/swimming changes it).
Updated 3 callers in entity scope: `tick_block_collisions`, `get_swim_height`, `get_eye_y`.
Remaining callers in command/ and player.rs left for future (would require touching non-entity files).

### 5. Fixed Cherry-Pick Build Error
The `528d5b3` cherry-pick introduced a type mismatch: `CollisionShape` vs `BoundingBox` in play.rs block placement check. Fixed by constructing `BoundingBox` from `CollisionShape` min/max fields.

## Decisions Made
- **ENT-019:** Cherry-pick all 5 "zero conflict" upstream commits immediately. These are low-risk bug fixes that improve vanilla parity without touching our agent work.
- **ENT-020:** Multi-conflict upstream ports (collision, totem, item drop) require cross-agent coordination. Documented needs in session log for Architect to coordinate.
- **ENT-021:** Vanilla parameter alignment: FollowParentGoal search=8, stop=9; PanicGoal flee=±5. These were silently wrong (2x vanilla) and would cause visibly non-vanilla behavior.

## Files Modified (7 directly, 5 cherry-picked)
- `pumpkin/src/entity/tnt.rs` — TNT fuse underflow fix
- `pumpkin/src/entity/ai/goal/creeper_ignite.rs` — stop() resets fuse_speed
- `pumpkin/src/entity/ai/goal/follow_parent.rs` — vanilla parameters (search=8, stop=9)
- `pumpkin/src/entity/ai/goal/panic.rs` — vanilla flee range (±5)
- `pumpkin/src/entity/mod.rs` — get_eye_height() + updated callers
- `pumpkin/src/entity/living.rs` — get_swim_height() uses get_eye_height()
- `pumpkin/src/net/java/play.rs` — CollisionShape→BoundingBox conversion fix

## What Others Should Know
- **Architect**: Collision fix (e438e09) needs AtomicF32 in pumpkin-util + BoundingBox::full_block(). Entity-scope changes ready when infra lands.
- **Block/Redstone agent**: Collision fix also needs get_inside_collision_shape trait method in block/mod.rs + registry.rs.
- **5 new goals needed**: EatGrass, Revenge, Beg, OwnerHurtByTarget, OwnerHurtTarget — all require new Mob trait hooks or LivingEntity fields.
- **Upstream AI overlap**: 4 commits safe to skip, 4 need selective merge, 5 must be incorporated. Full audit in this log.

## What I Need From Others
### Architect
- AtomicF32 in pumpkin-util (for collision fix e438e09)
- BoundingBox::full_block() method in pumpkin-util
- LivingEntity fields: last_attacker_id/time, last_attacking_id/time (for RevengeGoal + OwnerHurt goals)

### Block/Redstone Agent
- get_inside_collision_shape trait method + registry method (for collision fix)

## Build Status
- `cargo build` — clean
- `cargo clippy --all-targets --all-features` with `-Dwarnings` — clean
