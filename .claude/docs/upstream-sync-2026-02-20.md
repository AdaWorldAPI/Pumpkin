# Upstream Sync Analysis & Agent Task Split Recommendation

**Date:** 2026-02-20
**Analyst:** WorldGen agent
**Upstream:** https://github.com/Pumpkin-MC/Pumpkin (commit `33cb4e5`)
**Fork HEAD:** `7a73b15` (AdaWorldAPI/Pumpkin master)
**Divergence point:** `abe01f3`

---

## 1. Divergence Summary

| Metric | Value |
|--------|-------|
| Upstream-only commits | 94 |
| Fork-only commits | ~230 (agent work + PSScript) |
| Conflicting files (both sides modified) | ~200 |
| Clean cherry-picks (of 40 tested) | **5** |
| 1-conflict cherry-picks | 14 |
| Multi-conflict cherry-picks | 21 |

### Upstream Change Magnitude (since divergence)
| Area | Files | Insertions | Deletions |
|------|-------|------------|-----------|
| Entity (AI, mobs, combat) | 64 | +6,905 | -540 |
| World (chunks, lighting, gen) | 73 | +5,077 | -2,933 |
| Protocol | 27 | +1,248 | -152 |
| Block (redstone, fluids, fire) | 35 | +1,109 | -623 |
| Net (networking) | 14 | +541 | -260 |

---

## 2. Clean Cherry-Picks (Zero Conflict)

These 5 commits apply cleanly onto our master with no modifications needed:

| # | Hash | Description | Files | Risk |
|---|------|-------------|-------|------|
| 1 | `8433f05` | Prevent mobs from attacking when already dead | mob/mod.rs (+5) | LOW |
| 2 | `3f170e4` | Entity really teleport instead of only packet | entity/mod.rs, player.rs | LOW |
| 3 | `528d5b3` | Check every hitbox on block placement, not just player | block/registry.rs, entity | LOW |
| 4 | `a653726` | Preserve knockback velocity on killing blow | living.rs (-4) | LOW |
| 5 | `ba40fcd` | playersSleepingPercentage gamerule support | world/mod.rs (+13,-2) | LOW |

**Recommendation:** Cherry-pick all 5 immediately. Zero risk.

---

## 3. 1-Conflict Cherry-Picks (Manual Resolution Needed)

These have exactly 1 conflicting file each. Sorted by impact/effort:

### Tier A — High-value, easy resolve
| Hash | Description | Conflict File | Effort |
|------|-------------|---------------|--------|
| `11545cf` | Prevent subtraction overflow in translation_to_pretty | pumpkin-util/src/translation.rs | 5 min |
| `0acb913` | Sweet berry bush collision fix | sweet_berry_bush.rs | 5 min |
| `ab1926f` | Trapdoor placing fix (vanilla rotation) | trapdoor.rs | 5 min |
| `b0dfe19` | Fire loop/crash fix | fire/fire.rs | 10 min |
| `ef98000` | Fix log filename rotation | logging.rs | 5 min |
| `6eddc17` | Fence gate redstone power support | fence_gates.rs | 10 min |

### Tier B — Moderate value, player.rs conflicts
| Hash | Description | Conflict File | Effort |
|------|-------------|---------------|--------|
| `582d465` | Prevent stale chunk_sent entries blocking visibility | player.rs | 15 min |
| `73fda66` | Add fallback when chunk ACK stalls | player.rs | 15 min |
| `cb60cbc` | Resend replaced chunk data at same position | player.rs | 15 min |
| `ac9581f` | Void damage fix | player.rs | 10 min |
| `ae3aba1` | Align player pose with vanilla (prevent crawl-in-wall) | player.rs | 10 min |
| `5f13aa7` | Avoid drop item lock inversion with screen handler | player.rs | 10 min |
| `dfb24bf` | Projectile position regression fix | projectile/mod.rs | 10 min |

### Tier C — Lower priority or harder
| Hash | Description | Conflict File | Effort |
|------|-------------|---------------|--------|
| `854d9ce` | Creative drag + middle click in chests | screen_handler.rs | 15 min |

**Note:** The player.rs conflicts (Tier B) will compound — resolving them in sequence means later ones get harder. Best done in a single focused session.

---

## 4. Multi-Conflict (Manual Re-implementation Recommended)

These have 2+ conflicts and should be manually re-implemented by reading the upstream diff:

| Hash | Description | Conflicts | Best Agent |
|------|-------------|-----------|------------|
| `ad77d65` | Fix constant CPU load after player join | 4 | **Core** |
| `e438e09` | Collision fix | 5 | **Entity** |
| `5a5b9e4` | Totem behavior fix | 6 | **Entity** |
| `7e277c0` | Water flow not spreading correctly | 3 | **Redstone** |
| `9257b90` | Waterlogged blocks not recognized as water sources | 3 | **Redstone** |
| `6e1bea7` | Multiple fluid flow bugs | 6 | **Redstone** |
| `b22e667` | Preserve protocol ordering for sound event IDs | 2 | **Protocol** |
| `e75e63a` | Double-click behavior in chests | 2 | **Items** |
| `59b7e0a` | Spectator explosion immunity + TNT fuse underflow | 2 | **Entity** |
| `15fbf8f` | Item drop velocity | 5 | **Entity** |
| `fb0750c` | Inventory saving/loading | 13 | **Items+Storage** |
| `ba30b4e` | Close container screens when block destroyed | 12 | **Items** |

---

## 5. Upstream Features Worth Porting

### Must-Have (critical for vanilla parity)
| Hash | Description | Size | Agent | Effort |
|------|-------------|------|-------|--------|
| `8aeeacf` | Lighting system + refactored chunk_system | LARGE | **WorldGen** | 2-3 sessions |
| `8b6c8bb` | Basic pathfinder impl from vanilla | LARGE | **Entity** | 1-2 sessions |
| `9f78d65` | Packet encode/decode + compression perf | MEDIUM | **Protocol** | 1 session |
| `cef8a02` | BlockState Remapping | MEDIUM | **Architect** | 1 session |
| `24ea53b` | Replace log with tracing | LARGE | **Core** | 1 session |

### Nice-to-Have
| Hash | Description | Size | Agent |
|------|-------------|------|-------|
| `b77d9c1` | Complete boat implementation | MEDIUM | **Entity** |
| `ce46bf8` | Creeper AI, explosion, entity interaction | MEDIUM | **Entity** |
| `a9c8ab5` | World names | SMALL | **Core** |
| `3a9468d` | Cobweb block | SMALL | **Redstone** |
| `bc8ffa2` | Wither rose collision | SMALL | **Redstone** |
| `bb75b78` | Trapped chest redstone | SMALL | **Redstone** |
| `581437e` | Blender biome supplier hook | SMALL | **WorldGen** |
| `501eb25` | Firework data components | SMALL | **Items** |
| `74ecc3b` | Dynamic entity eye height | SMALL | **Entity** |
| `a96eca4` | Custom Payload (plugin) | MEDIUM | **Plugin** |
| `74e1cd1` | Entity interaction + block placement events | SMALL | **Plugin** |
| `33cb4e5` | Permission check event | SMALL | **Plugin** |
| `97951df` | Match campfires with vanilla logic | SMALL | **Items** |
| `58fdb0c` | Armor stand damage handling | SMALL | **Entity** |

### Entity AI (upstream rewrote mob AI — conflicts with our 81-mob system)
Upstream added 14 AI-related commits (chicken, cow, sheep, bat, cat, creeper, breed goal, follow parent, follow owner, beg goal, owner hurt goals, avoid entity goal, more entities). These **completely overlap** with our Entity agent's work (81 types, 21 goals). Direct cherry-pick is impossible — they must be **audited for vanilla logic differences** and selectively ported.

---

## 6. Recommended Strategy: Two-Phase Approach

### Phase 1: Immediate Safe Fixes (This Session)
**Agent: Any (WorldGen can do it)**
1. Cherry-pick the 5 clean commits
2. Manually resolve Tier A 1-conflict fixes (6 commits, ~40 min)
3. Test, commit, push

### Phase 2: Agent-Split Deep Sync (Multiple Sessions)

| Agent | Tasks | Priority | Estimated Sessions |
|-------|-------|----------|-------------------|
| **Core** | CPU load fix (`ad77d65`), `log` → `tracing` migration, world names, sleeping percentage | P0 | 1-2 |
| **Entity** | Collision fix, totem fix, TNT/spectator fix, item drop velocity, mob-dead-attack, pathfinder port, eye height, boat impl, AI audit vs upstream | P0 | 2-3 |
| **WorldGen** | Lighting system port (`8aeeacf`), blender biome hook, chunk system refactor | P0 | 2-3 |
| **Protocol** | Sound event ordering, packet perf, velocity encoding | P1 | 1 |
| **Redstone** | Water flow fixes (3 commits), cobweb, wither rose, fence gate, trapped chest | P1 | 1-2 |
| **Items** | Inventory saving/loading, chest slot fixes, container screen close, campfires, fireworks | P1 | 1-2 |
| **Plugin** | Custom payload, entity interaction events, permission check event | P2 | 1 |
| **Storage** | level.dat deserialization fix (`a61238a`) | P2 | 0.5 |
| **Architect** | BlockState remapping, coordinate full sync, update prompts | P1 | 1 |

---

## 7. Updated Agent Prompts (Debt + Continuation)

### What Changed Since Last Prompts
1. **94 upstream commits** have landed that our fork doesn't have
2. **PSScript commits** (15) still on our master need cleanup (see psscript-audit-2026-02-11.md)
3. **player.rs** is the biggest conflict hotspot (6+ upstream fixes all touch it)
4. **Lighting system** is the single biggest upstream feature we're missing
5. **Entity AI** upstream rewrote mob AI independently — needs careful merge with our 81-type system

### Prompt Updates Needed Per Agent

#### WORLDGEN Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: Lighting system from upstream `8aeeacf` (LARGE — new lighting/ module + chunk_system refactor)
- PORT: Blender biome supplier hook from `581437e` (SMALL — single file)
- AUDIT: chunk_system.rs has diverged significantly (upstream: +5077/-2933 in pumpkin-world/)
- The chunk system was refactored upstream alongside lighting — consider porting both together
```

#### ENTITY Prompt Addition:
```
## Upstream Debt (2026-02-20)
- CHERRY-PICK: `8433f05` (mobs attacking when dead), `a653726` (knockback on kill), `3f170e4` (teleport)
- PORT: Collision fix `e438e09` (5 conflicts), totem fix `5a5b9e4` (6 conflicts)
- PORT: Pathfinder from vanilla `8b6c8bb` (LARGE — replaces our basic pathfinding)
- PORT: Boat implementation `b77d9c1`, creeper AI `ce46bf8`, eye height `74ecc3b`
- AUDIT: Upstream added 14 mob AI commits (chicken/cow/sheep/bat/cat/creeper/breed/follow/beg/avoid)
  Our system has 81 types + 21 goals. Audit for vanilla logic we're missing, don't duplicate.
- FIX: Spectator explosion + TNT fuse `59b7e0a`, item drop velocity `15fbf8f`
```

#### CORE Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: CPU load fix `ad77d65` (4 conflicts — constant core load after player join)
- PORT: `log` → `tracing` migration `24ea53b` (LARGE — replaces logging framework)
- CHERRY-PICK: `ba40fcd` (sleeping percentage gamerule — clean)
- PORT: World names `a9c8ab5` (3 conflicts)
```

#### PROTOCOL Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: Packet encode/decode/compression perf `9f78d65` (4 conflicts)
- PORT: Sound event ID ordering `b22e667` (2 conflicts)
- PORT: Velocity encoding fixes `2ee194b` + `48b3d67` + `3cbf788`
```

#### REDSTONE Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: Water flow fix `7e277c0` (3 conflicts), waterlogged blocks `9257b90` (3 conflicts)
- PORT: Multiple fluid flow bugs `6e1bea7` (6 conflicts)
- PORT: Fence gate redstone `6eddc17` (1 conflict), trapped chest `bb75b78` (4 conflicts)
- PORT: Piston ghost blocks fix `7d10824`
- PORT: Cobweb `3a9468d`, wither rose collision `bc8ffa2`
- PORT: Lava and fire behavior `b3fc910`
```

#### ITEMS Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: Inventory saving/loading `fb0750c` (13 conflicts — LARGE)
- PORT: Close container on block destroy `ba30b4e` (12 conflicts — LARGE)
- PORT: Double-click slot fix `e75e63a` (2 conflicts)
- PORT: Creative drag + middle click `854d9ce` (1 conflict)
- PORT: Campfire vanilla logic `97951df`
- PORT: Firework data components `501eb25`
```

#### PLUGIN Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: Custom Payload `a96eca4` (5 conflicts)
- PORT: Entity interaction + block placement events `74e1cd1`
- PORT: Permission check event `33cb4e5`
```

#### STORAGE Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: level.dat deserialization for imported worlds `a61238a`
```

#### ARCHITECT Prompt Addition:
```
## Upstream Debt (2026-02-20)
- PORT: BlockState Remapping `cef8a02` (touches pumpkin-data)
- COORDINATE: Full upstream sync across all agents
- DECIDE: Whether to do a full rebase onto upstream vs selective porting
- UPDATE: All agent prompts with upstream debt items
```

---

## 8. The Big Decision: Rebase vs Selective Port

### Option A: Full Rebase onto upstream/master
- **Pro:** Gets all 94 fixes/features at once, stays in sync
- **Con:** ~200 file conflicts, would take 1-2 days of manual resolution
- **Risk:** HIGH — could break agent work, especially entity AI overlap

### Option B: Selective Port (Recommended)
- **Pro:** Controlled, testable, agent-scoped
- **Con:** Slower, may miss subtle fixes
- **Risk:** LOW — each agent ports their domain independently

### Option C: Hybrid — Rebase then cherry-pick agent work back
- **Pro:** Gets upstream as new base, re-applies our additions
- **Con:** Our 230 commits are mostly additive and may not replay cleanly
- **Risk:** MEDIUM

**Recommendation: Option B (Selective Port)** — Split upstream debt across agents as shown in Section 6. Each agent reads the upstream diff for their domain and manually implements the fixes. This preserves our existing work and is testable per-agent.

---

## 9. Immediate Action Items

1. **NOW:** Cherry-pick 5 clean fixes onto master
2. **NOW:** Update all 9 agent prompts with upstream debt sections
3. **NEXT SESSION:** Each agent picks up their P0 items from Section 6
4. **PSScript cleanup:** Still pending from psscript-audit-2026-02-11.md

---

## Appendix: All 94 Upstream Commits Categorized

### Bug Fixes (32)
ad77d65, e438e09, 582d465, 73fda66, cb60cbc, 8433f05, 3f170e4, 5a5b9e4,
b0dfe19, 7e277c0, 5f13aa7, ac9581f, ae3aba1, 0acb913, 528d5b3, b22e667,
ba30b4e, ab1926f, 11545cf, dfb24bf, e75e63a, a653726, 59b7e0a, 15fbf8f,
854d9ce, fb0750c, 9257b90, 6e1bea7, 58fdb0c, 97951df, a61238a, ef98000

### Features (17)
8aeeacf, 3a9468d, bc8ffa2, 6eddc17, bb75b78, a96eca4, 581437e, 501eb25,
b77d9c1, ba40fcd, a9c8ab5, cef8a02, 74ecc3b, 74e1cd1, 33cb4e5, 9435bbf,
2d40355+f8e0547+67f65e1 (player samples)

### Entity AI (14)
392d181, 8bd5009, ad96041, c0aea8c, ce46bf8, 05763ea, bce7359, 68f90e5,
45b1bf7, 296b8c2, 654dbc7, 41b4f28, d026ca7

### Performance (3)
9f78d65, f60a77c, e45ab61

### Infrastructure/Logging (4)
24ea53b, 8b6c8bb, 7d10824, b3fc910

### Chores/Deps/Docs (14)
1c4ea3a, 3a48274, 60c6332, 8995214, 1cf5ac6, 93997a9, b0ab500, 6aa6024,
8015b73, 473bd5a, dcda0d6, 88a38f7, 380ed85, 0efc051

### Velocity Fixes (4)
62c40dc, 0efc051, 2ee194b, e47c0fa+48b3d67+3cbf788
