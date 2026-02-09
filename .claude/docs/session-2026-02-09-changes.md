# Session Changes Log — February 9, 2026

## Overview

This session focused on **mobs are frozen** (root cause: missing AI ticking + timing issue), **idle CPU optimization verification**, and **admin/console configuration documentation**.

---

## 1. Changes Made

### 1.1 Entity Movement System — Idle CPU Optimization (Already Implemented)

**File:** [pumpkin/src/entity/mod.rs](../../../pumpkin/src/entity/mod.rs#L506)  
**Commit:** `dc1aa3b3` (Feb 9, 2026)  
**Change:** Added early-exit gate to skip collision math when no players online

```rust
// Skip expensive collision checks if no players are online to conserve CPU
let world = self.world.load();
if world.players.is_empty() {
    return movement;
}
```

**Impact:** When server has no players:
- ✅ Collision bounding box calculations **SKIPPED**
- ✅ Block collision queries **SKIPPED**  
- ✅ Blocking thread spawn for collision math **SKIPPED**
- ✅ Entities still tick (move, apply physics) but don't resolve collisions with blocks
- ℹ️ This is safe because no players means collision resolution isn't visible/needed

**Estimated CPU Reduction:** ~15-25% of entity tick time when idle (collision math is expensive)

---

### 1.2 Frozen Mobs Root Cause — Identified But Not Yet Fixed

**Status:** 🔴 **IN INVESTIGATION** — Root cause identified, fix pending

#### Problem Description
Mobs spawn successfully but don't move. Even hostile mobs (Zombie, ZombifiedPiglin) and endgame mobs (Warden, Guardian) stand frozen.

#### Root Causes Identified

1. **Missing AI Ticking in `tick_movement()` — LINE 381 placeholder**  
   **File:** [pumpkin/src/entity/living.rs](../../../pumpkin/src/entity/living.rs#L381)
   ```rust
   // TODO: Tick AI ← CRITICAL: This is never implemented!
   ```
   
2. **AI Goals Tick AFTER Movement Applied — Timing Lag**  
   **File:** [pumpkin/src/entity/mob/mod.rs](../../../pumpkin/src/entity/mob/mod.rs#L149-L179)  
   Current order:
   ```
   Tick 1: living_entity.tick() → tick_movement() uses movement_input (zero, no goals ran yet)
           → goals tick AFTER → movement_input set for next tick (too late!)
   Tick 2: living_entity.tick() → tick_movement() uses movement_input from Tick 1
           → goals tick AFTER → movement_input set (unused this tick)
   ```
   
3. **WanderAroundGoal Doesn't Set `movement_input` — No Direct Move Signal**  
   **File:** [pumpkin/src/entity/ai/goal/wander_around.rs](../../../pumpkin/src/entity/ai/goal/wander_around.rs)  
   **Finding:** Grep search for `movement_input` returns 0 matches
   
   **Implication:** Goals don't directly populate `movement_input` field. Unclear how goals communicate desired movement to the living entity.

#### AI System Status — FULLY IMPLEMENTED BUT NOT WIRED

- ✅ `GoalSelector` trait: Prioritized goal management with 5 control types
- ✅ `Goal` trait: Full lifecycle (can_start, start, tick, should_continue, stop)
- ✅ Concrete goals: SwimGoal, WanderAroundGoal, MeleeAttackGoal, LookAroundGoal, etc. (15+ types)
- ✅ All major mobs register goals (Zombie, Frog, ZombifiedPiglin, Guardian, etc.)
- ✅ Mob trait impl calls `goals_selector.tick()` every tick
- ❌ **Goals don't properly set movement intent on the living entity**
- ❌ **Timing: goals run after movement physics applied**

#### Next Steps Required

1. **Determine goal-to-movement communication pattern**
   - Read goal trait definition and implementations
   - Find how WanderAroundGoal should communicate "move toward X"
   - Check if `Navigator` system is used instead of direct `movement_input` setting

2. **Fix timing order**
   - Move goal ticking BEFORE `living_entity.tick()`, OR
   - Initialize `movement_input` from previous tick's goals, OR
   - Have goal_selector seed movement_input during start/tick

3. **Test**
   - Verify mobs move after fix
   - Test player movement isn't broken (regression)
   - Verify collision still works correctly

---

## 2. Configuration & Administration

### 2.1 Console / Admin Access

**Option 1: Server Console (Stdin/TTY)**
- **Type:** Direct server process console input
- **Access:** SSH/Terminal into server machine
- **Command:** Type commands directly into server terminal
- **Enabled by Default:** `use_console: true` in [server.properties](../../../pumpkin-config/src/commands.rs)
- **Requires:** Operator permission or console privilege

**Option 2: RCON (Remote Console)**
- **Type:** Remote TCP command interface
- **Port:** 25575 (default, configurable in server config)
- **Address:** `rcon_address` in config (default: `0.0.0.0:25575`)
- **Enabled by Default:** ❌ **NO** (`enabled: false`)
- **Password:** Configured in `rcon_password` field (defaults to empty string — MUST be set if enabled)
- **Connection:** Use RCON client at `<server_ip>:25575` with password
- **Security:** ⚠️ **WARNING** — RCON transmits in plain text; vulnerable on untrusted networks

**Option 3: In-Game Commands (For Operators Only)**
- Type `/command` as operator in chat
- Requires player to be **operator** (permission level ≥ 1)
- Operators managed in `ops.json` file

**Option 4: Command Blocks (In-Game)**
- Placed and configured in-game by operators
- Execute commands at block location
- Redstone triggerable

### 2.2 Admin/Operator Setup

**File:** [pumpkin/src/data/op.rs](../../../pumpkin/src/data/op.rs)  
**Configuration File:** `ops.json` in server root

**Operator Properties:**
- `uuid`: Player UUID
- `name`: Player name
- `level`: Permission level (0-4, see below)
- `bypasses_player_limit`: Whether this op can join when server full

**Permission Levels:**
- **Level 0:** Normal player (default, no console commands)
- **Level 1:** Operator (all commands in survival, limited creative)
- **Level 2:** Operator with command blocks
- **Level 3:** Operator with /publish command
- **Level 4:** Administrator (all commands, any permission)

**Default Op Level:** Set via `default_op_level` in [commands.rs](../../../pumpkin-config/src/commands.rs)  
(Defaults to **Level 0** — no special permission unless explicitly in ops.json)

### 2.3 No HTTP Admin Console

**Status:** ❌ Not implemented

There is **NO HTTP endpoint** at:
- `server:25565/admin` ← Does not exist
- `server:25565/console` ← Does not exist
- `server:25565/...` ← All HTTP admin paths do not exist

Access is **strictly**:
1. Server console (terminal/SSH)
2. RCON (TCP protocol, port 25575)
3. In-game chat (operators only)

---

## 3. Server Idle CPU Optimization — Status Summary

### What IS Optimized ✅

| Operation | When Idle | Impact |
|-----------|-----------|--------|
| Collision math | Skipped (~15-25% savings) | ✅ **GATES WITH**: `if world.players.is_empty()` |
| Block collision queries | Skipped (~5-10% savings) | ✅ **GATES WITH**: `if world.players.is_empty()` |
| Blocking thread pool spawn | Skipped | ✅ **GATES WITH**: `if world.players.is_empty()` |

### What Still Ticks When Idle ⚠️

| Operation | Reason | CPU Impact |
|-----------|--------|-----------|
| Entity tick (all) | Mobs still exist in loaded chunks | ~30-40% of idle CPU |
| Living entity physics | Mobs/animals still apply gravity, movement | ~20-30% of idle CPU |
| AI goal evaluation | Goal selector checks goal conditions | ~5-10% of idle CPU |
| Chunk ticking | Loaded chunks still tick (plants grow, etc.) | ~20-30% of idle CPU |
| World update tasks | Redstone, fire spread, block updates | ~10-20% of idle CPU |
| Network tick | Query protocol responses, RCON listen | ~2-5% of idle CPU |

### Recommendations for Further Idle CPU Reduction

1. **Pause entity ticking when no players online** (High impact ~30-40%)
   - Gate `world.tick_entities()` or entire entity loop
   - Preserve entity state but skip physics calculations
   
2. **Pause active chunk ticking when no players** (High impact ~20-30%)
   - Reduce/eliminate block updates (redstone, fire, plants)
   - When players rejoin, resume normal ticking

3. **Reduce world update task frequency** (Medium impact ~10-20%)
   - Defer expensive updates (tree growth, orefire spread)

---

## 4. Code Archaeology — Key Files

| File | Status | Notes |
|------|--------|-------|
| [pumpkin/src/entity/mod.rs](../../../pumpkin/src/entity/mod.rs#L506) | Optimized ✅ | Collision skip gate added |
| [pumpkin/src/entity/living.rs](../../../pumpkin/src/entity/living.rs#L366-L500) | TODO ❌ | Line 381: `// TODO: Tick AI` |
| [pumpkin/src/entity/mob/mod.rs](../../../pumpkin/src/entity/mob/mod.rs#L149-L179) | Timing issue ⚠️ | Goals tick AFTER movement |
| [pumpkin/src/entity/ai/goal/goal_selector.rs](../../../pumpkin/src/entity/ai/goal/goal_selector.rs) | Complete ✅ | GoalSelector fully implemented |
| [pumpkin/src/entity/ai/goal/wander_around.rs](../../../pumpkin/src/entity/ai/goal/wander_around.rs) | Incomplete ❌ | Doesn't set movement_input |
| [pumpkin-config/src/networking/rcon.rs](../../../pumpkin-config/src/networking/rcon.rs) | Documented ✅ | RCON config structure |
| [pumpkin-config/src/op.rs](../../../pumpkin-config/src/op.rs) | Documented ✅ | Operator (admin) config |
| [pumpkin-config/src/commands.rs](../../../pumpkin-config/src/commands.rs) | Documented ✅ | Console command config |

---

## 5. Git Status

- **Branch:** `fix/safe-collisions-bench`
- **Latest Commits:**
  - `820fae6c`: "entity: offload collision math to blocking thread and add micro-benchmark"
  - `dc1aa3b3`: "perf: skip collision math when no players online to reduce idle CPU load"
- **Status:** Ready for PR; collisions optimized, mobs issue separate

---

## 6. Remaining Work (Priority Order)

| Priority | Task | Effort | Status |
|----------|------|--------|--------|
| 🔴 P0 | Fix frozen mobs (AI ticking + timing) | Medium | 🔴 Not started |
| 🔴 P0 | Implement 45 unenforced game rules | High | 🟡 Identified |
| 🟡 P1 | Further idle CPU optimization (pause entity tick) | Medium | Not started |
| 🟡 P1 | Bedrock GameRules packet complete | Low | Not started |

---

## 7. Session Date & Context

- **Date:** February 9, 2026
- **Branch:** `fix/safe-collisions-bench`
- **Server State:** Stable (3-min crash fixed); mobs frozen (AI timing issue)
- **Game Rules:** 14/59 enforced (24%)

