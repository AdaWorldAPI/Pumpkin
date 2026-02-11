# PSScript Commit Audit & Rollback/Redo Guide

**Date:** 2026-02-11
**Auditor:** WorldGen agent
**Scope:** 15 non-agent commits by PSScript on 2026-02-09/10
**Baseline:** `6f23bc5` (last known good master, Sunday night)
**Current HEAD:** `7a73b15`

---

## Executive Summary

15 commits were pushed to master by PSScript (VS Code / Codex / Copilot).
The server was "almost perfect" at the Sunday baseline -- only mob movement
issues. These commits introduced a mix of genuine bugfixes, performance
experiments, and massive formatting churn. Several changes broke game mechanics.

**Verdict:** Roll back to `6f23bc5`, then cherry-pick the good fixes cleanly.

| Category | Count | Action |
|----------|-------|--------|
| KEEP (genuine bugfixes) | 5 changes | Cherry-pick |
| DISCARD (broke game mechanics) | 3 changes | Do not re-apply |
| REDO (good idea, bad execution) | 2 changes | Rewrite cleanly |
| NOISE (formatting, line endings) | 5 commits | Do not re-apply |

---

## Rollback Command

```bash
# Reset master to Sunday baseline
git checkout master
git reset --hard 6f23bc5
git push --force-with-lease origin master
```

This removes all 15 PSScript commits. Then cherry-pick the good parts below.

---

## KEEP -- Cherry-pick These Fixes

### 1. Mob tick ordering (ac14f43)

**File:** `pumpkin/src/entity/mob/mod.rs`
**What:** Move `mob_entity.living_entity.tick(caller, server).await` from
BEFORE AI goals to AFTER AI goals/navigation/look_control.

```rust
// In MobEntity::tick(), move this line from the top to the bottom:
mob_entity.living_entity.tick(caller, server).await;
// It should run AFTER:
//   mob_entity.goal_selector.tick(...)
//   mob_entity.target_selector.tick(...)
//   mob_entity.navigation.tick(...)
//   mob_entity.look_control.tick(...)
```

**Why keep:** Matches vanilla -- AI sets intent, then physics applies it same tick.
Fixes the 1-tick pathfinding lag you saw with mob movement.

---

### 2. Fire re-scheduling fix (part of 8fabaf4)

**File:** `pumpkin/src/block/blocks/fire/fire.rs`
**What:** Move `schedule_block_tick` from top of `on_scheduled_tick` to bottom,
guarded by a check that the fire block still exists.

```rust
// REMOVE from top of on_scheduled_tick:
//   world.schedule_block_tick(block, *pos, Self::get_fire_tick_delay(), ...).await;

// ADD at bottom of on_scheduled_tick, after all extinguish logic:
let current_block = world.get_block(pos).await;
if current_block.id == block.id {
    world.schedule_block_tick(block, *pos, Self::get_fire_tick_delay(),
        TickPriority::Normal).await;
}
```

**Why keep:** Prevents ghost fire ticks on extinguished blocks.

---

### 3. Totem + damage cooldown fixes (part of 8fabaf4)

**File:** `pumpkin-world/src/item/mod.rs` -- Add `ItemStack::clear()`:
```rust
pub fn clear(&mut self) {
    *self = Self::EMPTY.clone();
}
```

**File:** `pumpkin/src/entity/living.rs` -- Three changes:

a) Replace `stack.decrement(1)` with `stack.clear()` in totem consumption

b) Add damage cooldown bypass for `/kill` and void:
```rust
let bypasses_cooldown_protection =
    damage_type == DamageType::GENERIC_KILL || damage_type == DamageType::OUT_OF_WORLD;

// In cooldown check:
if self.hurt_cooldown.load(Relaxed) > 10 && !bypasses_cooldown_protection {

// In death protector check:
if new_health <= 0.0
    && (bypasses_cooldown_protection || !self.try_use_death_protector(caller).await)
```

**Why keep:** All three match vanilla behavior. `/kill` must always work.

---

### 4. `is_part_of_game()` logic inversion fix (part of db9a2d5)

**File:** `pumpkin/src/entity/living.rs`

```rust
// OLD (broken):
pub fn is_part_of_game(&self) -> bool {
    self.is_spectator() && self.entity.is_alive()
}
// FIXED:
pub fn is_part_of_game(&self) -> bool {
    !self.is_spectator() && self.entity.is_alive()
}
```

**Why keep:** The old code was inverted -- only spectators were "in game".

---

### 5. Chunk scheduler dependency pre-check (82f01f1 + 7a73b15 net result)

**File:** `pumpkin-world/src/chunk_system.rs`
**What:** Before building a generation cache, check all dependency chunks exist.
If any is missing, defer the task.

```rust
// ADD before the cache-building loop:
let mut missing_dependency_chunk = false;
'check_chunks: for dx in -write_radius..=write_radius {
    for dy in -write_radius..=write_radius {
        let new_pos = node.pos.add_raw(dx, dy);
        let holder = self.chunk_map.get(&new_pos).unwrap();
        if holder.chunk.is_none() {
            missing_dependency_chunk = true;
            break 'check_chunks;
        }
    }
}
if missing_dependency_chunk {
    self.graph.nodes.remove(occupy);
    self.queue.push(task);
    break 'out2;
}
```

**Why keep:** Prevents panic when proto neighbor chunks are missing.
**Improvement needed:** Add `log::warn!` when deferring so livelock is detectable.

---

### 6. Unsafe removal in Bedrock networking (part of 77c3bf3)

**File:** `pumpkin/src/net/bedrock/mod.rs`
**What:** Replace 3 `unsafe { unwrap_unchecked() }` blocks with safe
`filter_map`/`match`/`let else` patterns. Add empty payload guard.

```rust
// Replace unsafe len sum:
let len: usize = frames.iter()
    .filter_map(|f| f.as_ref().map(|frame| frame.payload.len()))
    .sum();

// Replace unsafe merge:
for frame_opt in &frames {
    let Some(f) = frame_opt else {
        log::warn!("missing fragment in compound {compound_id}");
        return Ok(());
    };
    merged.extend_from_slice(&f.payload);
}

// Replace unsafe take:
let Some(f) = frames[0].take() else {
    log::warn!("missing single frame for compound {compound_id}");
    return Ok(());
};
frame = f;

// Add guard:
if frame.payload.is_empty() {
    log::warn!("Received empty Bedrock frame payload");
    return Ok(());
}
```

**Why keep:** Removes UB risk from network input handling.

---

### 7. Unwrap removal in collision/metadata (part of 77c3bf3)

**File:** `pumpkin/src/entity/mod.rs`
**What:** Replace `.unwrap()` with `let...else` in collision block iterator
and metadata serialization.

```rust
// Collision iterator:
let Some((mut collisions_len, mut position)) = positions.next() else {
    log::warn!("Empty block positions iterator in collision detection");
    return adjusted_movement;
};

// Metadata write:
if let Err(e) = metadata.write(&mut buf, &MinecraftVersion::V_1_21_11) {
    log::warn!("Failed to write entity metadata to buffer: {e}");
    return;
}
```

**Why keep:** Prevents server panics from malformed data.

---

### 8. SKnownPacks revert to upstream (edc4574)

**File:** `pumpkin-protocol/src/java/server/config/known_packs.rs`
**What:** Revert full pack deserialization to just `known_pack_count: VarInt`.
Remove `KnownPackEntry` struct.

**Why keep:** Matches upstream, less fragile, server doesn't use pack entries.

---

### 9. Dockerfile fixes (92be67e + be454cf + 0045297 + 888ae47)

**Net result:** Remove cache mounts from Dockerfile (Railway compat),
add Dockerfile to .gitignore.

**Why keep:** Infrastructure only, no Rust code impact.

---

## DISCARD -- Do NOT Re-apply

### X1. Collision skip when no players online

**File:** `pumpkin/src/entity/mod.rs` (from dc1aa3b, refined in e6a942d)

```rust
// DO NOT ADD THIS:
let world = self.world.load();
if world.players.load().is_empty() {
    return movement;  // <-- entities clip through blocks!
}
```

**Why discard:** ALL entities (mobs, falling blocks, items, minecarts) skip
collision when 0 players are connected. Entities in loaded chunks fall through
terrain. Vanilla does not do this. Breaks automated farms, falling sand,
hopper minecarts.

---

### X2. PUMPKIN_IDLE_TICK_MS env var / idle tick throttling

**File:** `pumpkin/src/server/ticker.rs` (from e6a942d)

```rust
// DO NOT ADD THIS:
fn idle_tick_interval() -> Option<Duration> { ... }
// Or the conditional:
} else if !server.has_n_players(1) {
    idle_tick_interval().unwrap_or_else(|| ...)
```

**Why discard:** Slows ALL game mechanics (redstone, mob AI, weather, daylight,
scheduled ticks) when no players online. Uses undocumented env var outside
config system. Cannot be changed at runtime (OnceLock). No validation
(1ms = 1000 TPS possible).

---

### X3. Misleading SAFETY comment on from_utf8_unchecked

**File:** `pumpkin-protocol/src/serial/deserializer.rs`

```rust
// DO NOT ADD THIS COMMENT:
// SAFETY: The Minecraft protocol guarantees that all string data is valid UTF-8.
```

**Why discard:** The comment is false. Client input is untrusted. The existing
`unsafe` is a latent bug; adding a misleading safety comment makes it look
justified. The real fix is replacing `from_utf8_unchecked` with `from_utf8`.

---

## REDO -- Good Idea, Rewrite Cleanly

### R1. Collision math extraction

**Original:** `820fae6` extracted inline collision math into
`compute_collision_math()` and offloaded it via `spawn_blocking`.

**What to keep:** The pure function extraction is good -- separates concerns.

```rust
pub fn compute_collision_math(
    movement: Vector3<f64>,
    bounding_box: BoundingBox,
    collisions: Vec<BoundingBox>,
    block_positions: Vec<(usize, BlockPos)>,
) -> (Vector3<f64>, Option<BlockPos>, bool) {
    // ... pure collision math, no async ...
}
```

**What to NOT keep:** The `spawn_blocking` wrapper. Each entity spawns a
blocking task every tick -- the scheduling overhead (thread pool handoff,
channel, context switch) likely exceeds the cost of the float math. Also,
`.unwrap_or((movement, None, false))` silently drops panic errors.

**Redo approach:** Keep the function extraction. Call it directly (not via
`spawn_blocking`). Add the benchmark properly with `[[bench]]` in Cargo.toml.

```toml
# Add to pumpkin/Cargo.toml:
[[bench]]
name = "collision_bench"
harness = false
```

---

### R2. New mob attack infrastructure (from db9a2d5)

**What was added:**
- `LivingEntity::swing_hand()` -- broadcasts arm swing animation
- `LivingEntity::can_take_damage()` -- checks invulnerable + spectator
- `MobEntity::is_in_attack_range()` -- bounding box intersection check
- `MobEntity::try_attack()` -- executes attack with hardcoded 3.0 damage
- `MobEntity::get_attack_box()` -- computes attack hitbox

**What to keep:** `swing_hand()`, `can_take_damage()`, `is_in_attack_range()`,
`get_attack_box()` -- all are reasonable additive code.

**What to fix:** `try_attack()` hardcodes `ZOMBIE_ATTACK_DAMAGE: f32 = 3.0`
for ALL mob types. Every mob deals zombie-level damage. Redo with entity
attribute system lookup.

---

## NOISE -- Pure Formatting, Do Not Re-apply

These commits change hundreds of files with zero behavioral impact:

| Commit | Description | Files | Action |
|--------|-------------|-------|--------|
| `77c3bf3` (partial) | Formatting across 52 .rs files | 52 | Skip |
| `5fade14` | Formatting across 27 .rs files | 27 | Skip |
| `9d8e225` | Idiomatic refactor of 77c3bf3's code | 2 | Included in KEEP #7 |
| `820fae6` (partial) | Line-ending normalization | ~200 | Skip |
| `c9c4931` | rustfmt on benchmark + entity | 2 | Skip |

These formatting changes touch your structure files (village.rs,
bastion_remnant.rs, trial_chambers.rs, etc.) but only reformat
whitespace. If you roll back to `6f23bc5`, your structure code returns
to its original formatting, which is fine.

---

## Cherry-Pick Order (after rollback)

If you roll back to `6f23bc5` and want to re-apply the good parts:

```bash
# 1. Mob tick ordering
#    Manual edit: pumpkin/src/entity/mob/mod.rs
#    Move living_entity.tick() to after AI goals

# 2. Fire rescheduling
#    Manual edit: pumpkin/src/block/blocks/fire/fire.rs
#    Move schedule_block_tick to bottom, add existence check

# 3. ItemStack::clear + totem + cooldown fixes
#    Manual edit: pumpkin-world/src/item/mod.rs (add clear())
#    Manual edit: pumpkin/src/entity/living.rs (3 changes)

# 4. is_part_of_game inversion fix
#    Manual edit: pumpkin/src/entity/living.rs (add !)

# 5. Chunk scheduler pre-check (net of 82f01f1 + 7a73b15)
#    Manual edit: pumpkin-world/src/chunk_system.rs
#    Add pre-check loop before cache building

# 6. Bedrock unsafe removal
#    Manual edit: pumpkin/src/net/bedrock/mod.rs (3 blocks)

# 7. Collision unwrap removal
#    Manual edit: pumpkin/src/entity/mod.rs (2 blocks)

# 8. SKnownPacks revert
#    cherry-pick edc4574

# 9. Dockerfile
#    cherry-pick 888ae47 (gitignore only)

# 10. Collision math extraction (REDO cleanly)
#     Extract compute_collision_math() as pure fn
#     Call directly, NOT via spawn_blocking
#     Add [[bench]] to Cargo.toml
```

---

## Files Affected by Rollback

Rolling back to `6f23bc5` removes changes from these Rust files:

**WorldGen-owned (will need re-push of agent work):**
- None -- all WorldGen commits were merged before `6f23bc5`

**Entity:**
- `pumpkin/src/entity/mod.rs` -- loses collision extraction + unwrap fixes
- `pumpkin/src/entity/mob/mod.rs` -- loses tick ordering + attack infra
- `pumpkin/src/entity/living.rs` -- loses totem/cooldown/is_part_of_game fixes

**Block:**
- `pumpkin/src/block/blocks/fire/fire.rs` -- loses fire rescheduling fix

**Protocol:**
- `pumpkin-protocol/src/serial/deserializer.rs` -- loses safety comments (good)
- `pumpkin-protocol/src/java/server/config/known_packs.rs` -- loses SKnownPacks revert

**Networking:**
- `pumpkin/src/net/bedrock/mod.rs` -- loses unsafe removal

**World (pumpkin-world):**
- `pumpkin-world/src/chunk_system.rs` -- loses chunk scheduler pre-check
- `pumpkin-world/src/item/mod.rs` -- loses ItemStack::clear()

**Server:**
- `pumpkin/src/server/ticker.rs` -- loses idle tick throttle (good)

**Benchmark:**
- `pumpkin/benches/collision_bench.rs` -- file removed (broken anyway)

---

## Impact of NOT Rolling Back

If you keep current master as-is, these issues persist:

1. **Entities clip through blocks** when server has 0 players
2. **Tick rate can slow arbitrarily** via undocumented env var
3. **Every entity spawns a blocking task** per tick for collision math
4. **Benchmark is broken** (no `[[bench]]` in Cargo.toml)
5. **All mobs deal 3.0 damage** regardless of type
6. **Misleading safety comment** on `from_utf8_unchecked`
7. **200+ files** have noisy line-ending diffs poisoning git blame

---

## Branch Inventory

All Claude agent branches still exist on remote and contain clean work:

| Branch | Agent | Status |
|--------|-------|--------|
| `claude/worldgen-terrain-biomes-P3zSp` | WorldGen | Current, rebased |
| `claude/entity-spawning-ai-V7oqj` | Entity | Merged to master |
| `claude/redstone-signal-propagation-QKEoc` | Redstone | Merged to master |
| `claude/items-agent-setup-cgzPo` | Items | Merged to master |
| `claude/core-agent-setup-IWqRa` | Core | Merged to master |
| `claude/plugin-api-events-5Q5l2` | Plugin | Merged to master |
| `claude/nbt-anvil-implementation-cmxPq` | Storage | Merged to master |
| `claude/protocol-packets-serialization-7c89s` | Protocol | Merged to master |
| `claude/architect-setup-LkWIY` | Architect | Merged to master |

All agent work was merged before `6f23bc5`, so rolling back to that
commit preserves ALL agent contributions.
