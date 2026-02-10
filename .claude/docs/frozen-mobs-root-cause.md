# Frozen Mobs Root Cause Analysis

## Executive Summary

**Bug:** Mobs spawn but don't move (completely frozen)
**Root Cause:** AI goal execution order is **reversed** compared to official repo
**Impact:** Goals set movement_input but it's never applied (always missed by physics tick)
**Severity:** CRITICAL - Breaks all mob behavior

---

## The Bug: Timing Order Reversal

### Official Repository (Working ✓)

File: `/pumpkin/src/entity/mob/mod.rs` lines ~200-240

```rust
impl<T: Mob> EntityBase for T {
    fn tick(...) -> EntityBaseFuture {
        Box::pin(async move {
            let mob_entity = self.get_mob_entity();
            
            // 1️⃣ AI GOALS RUN FIRST - set movement_input
            mobentity.target_selector.lock().await.tick(self).await;      
            mob_entity.goals_selector.lock().await.tick(self).await;
            
            // 2️⃣ NAVIGATOR processes path with AI movement
            navigator.tick(&mob_entity.living_entity).await;
            
            // 3️⃣ LOOK CONTROL finalizes head rotation
            look_control.tick(self).await;
            
            // 4️⃣ PHYSICS runs LAST - uses movement set by goals
            mob_entity.living_entity.tick(caller, server).await;
            
            // 5️⃣ Update rotation packets after everything is set
            // (yaw, pitch, head_yaw caching)
        })
    }
}
```

**Expected behavior:**
- Tick N: Goals set `movement_input = (1, 0, 0)` → Physics applies it immediately ✓
- Tick N+1: Goals set `movement_input = (1, 0, 0)` → Physics applies it immediately ✓

---

### Workspace Fork (Broken ✗)

File: `/workspaces/Pumpkin/pumpkin/src/entity/mob/mod.rs` lines ~155-195

```rust
impl<T: Mob> EntityBase for T {
    fn tick(...) -> EntityBaseFuture {
        Box::pin(async move {
            let mob_entity = self.get_mob_entity();
            
            // ❌ 1️⃣ PHYSICS runs FIRST - no movement_input set yet!
            mob_entity.living_entity.tick(caller, server).await;
            
            // ❌ 2️⃣ AI GOALS run AFTER movement already applied (TOO LATE!)
            mob_entity.target_selector.lock().await.tick(self).await;
            mob_entity.goals_selector.lock().await.tick(self).await;
            
            // ❌ 3️⃣ NAVIGATOR runs but can't affect this frame's movement
            navigator.tick(&mob_entity.living_entity).await;
            
            // ❌ 4️⃣ LOOK CONTROL updates head differently
            look_control.tick(self).await;
        })
    }
}
```

**Actual behavior (broken):**
- Tick 0: Physics applies `movement = (0, 0, 0)` [no prior goals] → Goals set `(1, 0, 0)` [too late!]
- Tick 1: Physics applies `movement = (0, 0, 0)` [goals don't help] → Goals set `(1, 0, 0)` [too late!]
- Tick 2: Physics applies `movement = (0, 0, 0)` [repeats infinitely] → Goals set `(1, 0, 0)`

**Result: Infinite loop of zero movement**

---

## Why This Breaks Mob AI

### Movement Input Pipeline

The entity movement system depends on this order:

```
Goal Tick
  ↓
Goal.tick() sets entity.movement_input = Vector3 (e.g., WanderAroundGoal)
  ↓
Goal.tick() uses navigator to process pathfinding
  ↓
Navigator.tick() applies movement_input to plan route
  ↓
LivingEntity.tick() reads movement_input and applies physics
  ↓
Entity moves ✓
```

### What Happens in Workspace (WRONG ORDER)

```
LivingEntity.tick() - No movement_input set yet (or stale from previous tick)
  ↓
Entity tries to move with zero/stale velocity ✗
  ↓
Goal.tick() - Sets movement_input AFTER physics already ran
  ↓
Navigator.tick() - Plans movement that won't be used until next tick
  ↓
Head Rotation - Updates head but body is frozen ✗
```

---

## Impact on All Systems

Despite optimizations added (collision offload, idle gate), they **don't help** because:

1. **No movement to collide with** - Always zero velocity, no collision math needed
2. **Idle optimization useless** - Gate is `if world.players.is_empty()` but issue is mob movement itself is broken
3. **Goals fully implemented** - AI system code is correct, just runs at wrong time
4. **Not a missing TODO** - Living.rs line 381 `// TODO: Tick AI` is deceiving (AI exists, timing is wrong)

---

## Data Changes Between Versions

The official repo also includes optimizations the workspace removed:

**Officially present (workspace deleted):**
```rust
last_sent_yaw: AtomicU8,
last_sent_pitch: AtomicU8,
last_sent_head_yaw: AtomicU8,
```

These cache last-sent rotation values to avoid duplicate packets.

**Officially uses:**
```rust
mb_entity.target_selector.lock().await.tick_goals(self, false).await;
```

(Limited goal ticking for performance)

**Workspace still uses:**
```rust
mob_entity.target_selector.lock().await.tick(self).await;
```

(Full goal ticking - not wrong, just less optimized)

---

## How This Happened

Timeline speculation:
1. Fork created from official repo
2. Dev refactored mob tick cycle for some reason (maybe trying to solve a different problem?)
3. Moved `living_entity.tick()` to first position
4. All subsequent testing likely done in creative mode or with players that override mob AI
5. Mob behavior broken but unnoticed until now

---

## Fix: Restore Correct Execution Order

The fix requires moving `living_entity.tick()` to **last**:

```rust
// ✓ CORRECT ORDER:
impl<T: Mob> EntityBase for T {
    fn tick(...) {
        // 1. AI goals first (set movement)
        target_selector.tick().await;
        goals_selector.tick().await;
        
        // 2. Navigation/look control
        navigator.tick().await;
        look_control.tick().await;
        
        // 3. Physics last (apply movement)
        living_entity.tick().await;
    }
}
```

This ensures movement_input set in step 1 is available for step 3.

---

## Verification Checklist

After fix, verify:
- [ ] Mobs spawn and wander naturally
- [ ] Zombies chase players
- [ ] Creepers walk toward targets
- [ ] Spiders avoid water
- [ ] Slimes bounce in place
- [ ] Flying mobs (phantoms, withers) move
- [ ] All AI goals working without one-tick lag

