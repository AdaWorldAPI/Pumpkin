# Shopping List Inventory — What Exists vs What Was Planned

**Date:** 2026-03-24
**Branch:** `claude/architect-setup-LkWIY` (rebased from `claude/compare-simd-implementations-btTgj`)
**Assessed against:** Three-session consolidated shopping list (14 hotspots + 3 zero-jitson fixes)

---

## Legend

| Rating | Meaning |
|--------|---------|
| **PERFECT** | Implemented, tested, integrated, no changes needed |
| **SOLID** | Exists and works but has specific improvement points |
| **PARTIAL** | Scaffolding or stubs exist, real logic is TODO |
| **GAP** | Designed/documented but zero implementation |
| **MISSING** | Not addressed at all — no code, no stub, no plan |

---

## A. Existing Infrastructure (pumpkin-store + overlays)

### A1. `pumpkin-store` crate — SOLID

**Files:** `pumpkin-store/src/{lib,traits,cached_store,static_store,lance_store,error}.rs`
**Tests:** 30+ tests, all passing

**What's perfect:**
- `GameDataStore` trait with 6 entity domains (block, item, entity, recipe, game_mapping)
- `StoreProvider` meta-switch (Static/Cached/Lance) with single-flip routing
- `CachedStore<S>` with `RwLock<HashMap>` memoization + XOR write-through guard
- `ZeroCopyGuard` trait — borrow_mask/xor_tag/verify_xor on all record types
- `CacheEntry<T>` transparent DTO with method/key metadata
- 26 tests covering: roundtrip, zero-copy preservation, XOR breach detection, serialization

**Points for improvement:**
- `CachedStore` uses `RwLock<HashMap>` per lookup method (5 separate locks). Under high contention this creates lock-convoy. Should consider `DashMap` or sharded locks for Phase 4.
- `clone()` on every cache hit — even with `Cow::Borrowed`, the record struct itself is cloned. Could return `Arc<CacheEntry<T>>` for true zero-copy reads.
- No cache eviction policy — grows unbounded. Fine for game registries (finite), but `game_mappings` could be large.
- `LanceStore` is a pure stub — every `GameDataStore` method returns `Err("not yet implemented")`. The `hydrate_from()` is a no-op. **Phase 4 is documented but zero real work.**

### A2. `SpatialOverlay` (2^14 Hamming vector) — SOLID

**File:** `pumpkin-store/src/traits.rs:246-413`
**Tests:** 12 tests

**What's perfect:**
- 256 × u64 = 16384-bit spatial hash with golden-ratio multiplicative constants
- `bind(x,y,z)` / `unbind(x,y,z)` → bit-level spatial tracking
- `xor_diff()` → activity detection between ephemeral + static tables
- `hamming_weight()` / `hamming_distance()` → change counting
- `as_words()` / `as_words_mut()` → raw access for SIMD interop

**Points for improvement:**
- `xor_diff()` and `hamming_distance()` use scalar loops over 256 u64 words. The doc **says** "AVX-512: 32 ops to diff entire table" but the implementation is pure scalar. This is the exact place where SIMD would help — `vpxorq zmm0,zmm1,zmm2` + `vpopcntq` per 512-bit lane.
- No SIMD feature gate or conditional dispatch exists.
- Hash function (`spatial_hash`) uses top 14 bits after XOR-shift. No avalanche test or collision analysis in tests.

### A3. `MobGoalState` (u64 bitpack) — PERFECT

**File:** `pumpkin-store/src/traits.rs:147-244`
**Tests:** 3 tests

Compact, correct, well-tested. No improvement needed — this is exactly what a 64-bit goal state encoder should look like. `xor_diff()` + `hamming_distance()` for state transitions is the right primitive.

### A4. `holograph.rs` (AI adapter) — PARTIAL

**File:** `pumpkin/src/entity/ai/holograph.rs` (76 lines)
**Tests:** None

Pure scaffold: `evaluate_holograph_tick_plan()` returns `(age + entity_id) % 2 != 0` — a dummy decision. Shadow mode toggle via env var. Runtime kill-switch via `AtomicBool`. No real AI/inference logic, no connection to `SpatialOverlay` or `MobGoalState`.

---

## B. Shopping List vs Reality — Per Item

### Tier 1: Do First

| # | Hotspot | Rating | What Exists | What's Missing |
|---|---------|--------|-------------|----------------|
| 1 | **Terrain fill loop** (98K/chunk, worldgen) | **MISSING** | No proto_chunk.rs found in this fork. Worldgen fills are in `pumpkin-world/src/world_gen/` but no batch/SIMD path. | No `Array1<f64>` fill, no batch noise API, no ndarray integration. The `SIMD_COMPARISON.md` proposes `sample_batch(&self, xs: &[f64; 4], ...)` but zero code exists. |
| 2 | **Entity spatial search** (O(n) per tick) | **GAP** | `SpatialOverlay` exists with 2^14 hashing. `world/mod.rs:2551-2580` has `get_nearby_players()` / `get_nearby_entities()` doing O(n) linear scan. | No spatial index (grid, k-d tree, octree) is actually wired in. `SpatialOverlay` is a *fingerprint* (activity detection) not a *spatial index* (neighbor query). The O(n) linear scan remains the production path. Need a real `SpatialHashGrid<EntityId>` with cell-based lookup. |
| 3 | **Palette unpack + count** (4096×24/chunk) | **GAP** | `palette.rs:141-213` does the unpack correctly. Lines 190-201 have the O(n×m) count-building: `palette.iter().position()` per decompressed value. | No SIMD unpack path. No batch count via histogram. The `position()` search is O(palette_len) per block = O(4096 × palette_len) per section. Should be a direct index count (since decompressed values ARE palette indices after unpack). |
| 4 | **Entity-player collision** (O(N×M) per tick) | **GAP** | `world/mod.rs` has entity iteration. No spatial acceleration. | Same root cause as #2. Without spatial index, every entity checks every player every tick. |

### Tier 2: Do Second

| # | Hotspot | Rating | What Exists | What's Missing |
|---|---------|--------|-------------|----------------|
| 5 | **Projectile ray-AABB** | **MISSING** | Projectile code exists but no batch ray-AABB intersection. | No `aabb.rs` module. No SSE4.1 slab test. No batch ray casting. |
| 6 | **Block collision gather** | **GAP** | `block_state.rs:145-162` has `get_block_collision_shapes()` / `get_block_outline_shapes()`. These return iterators over `COLLISION_SHAPES[id]`. | The async overhead is documented as the real bottleneck. No batch gather. The shape lookup itself is O(1) per shape ID. |
| 7 | **Palette pack** (Java + Bedrock) | **GAP** | `palette.rs:108-138` does pack with `.position()` linear search per block in the inner loop (line 125). | O(n) per block × 4096 blocks per section. Should use HashMap<V, usize> for O(1) palette index lookup. This is the "plain bug fix" category — not a SIMD target, just bad algorithm. |
| 8 | **Light nibble batch extract** | **MISSING** | No light storage or propagation exists beyond stubs. `format/mod.rs` mentioned in shopping list — no nibble pack/unpack module found. | No `nibble.rs`, no AVX2 4-bit pack/unpack, no light level storage. |
| 9 | **Vicinity tick distance** | **GAP** | `world/mod.rs:250-306` (approximate) does player distance checks. Linear scan. | Same spatial index gap. |

### Tier 3: Later / Niche

| # | Hotspot | Rating | What Exists | What's Missing |
|---|---------|--------|-------------|----------------|
| 10 | **Noise octave sampling** | **GAP** | `pumpkin-util/src/noise/perlin.rs` has full Perlin implementation. `SIMD_COMPARISON.md` Section 3.1 has detailed batch design. | Zero SIMD code. Pure scalar point-at-a-time. The analysis is excellent (gather bottleneck identified, fade/lerp vectorizable, 70% of cost is SIMD-friendly). But no `wide` or `pulp` crate, no `sample_batch()` function. |
| 11 | **Random tick batch count** | **SOLID** | `pumpkin-data/src/generated/block.rs:10-488` has 464-word u64 bitset. `has_random_ticks()` is `#[inline(always)]` const fn with O(1) lookup. Benchmark exists (`benches/has_random_ticks.rs`). | Single lookups are already optimal. Batch popcount (how many in a section?) would benefit from SIMD `vpopcntq` but this is low-priority since the current approach only checks 3 random positions per section. |
| 12 | **NBT batch scan** | **MISSING** | NBT crate (`pumpkin-nbt/`) exists for parsing. No batch varint decode, no length-prefixed protocol parser. | No `byte_scan.rs` module. I/O-dominated anyway — CPU portion is small. |
| 13 | **EndIsland density** | **MISSING** | No End dimension generation found in this fork. | Niche, End-only. |
| 14 | **Light BFS propagation** | **MISSING** | No propagation engine. Light data is stubs only. | Speculative — can't optimize what doesn't exist yet. |

### Zero-Jitson Fixes

| Fix | Rating | Current State | What to Do |
|-----|--------|---------------|------------|
| **`is_waterlogged()`** | **GAP (bug)** | `block_state.rs:134-143`: Calls `block.properties(self.id)` → `to_props()` → `Vec<(&str, &str)>` → `.any(k == "waterlogged" && v == "true")`. Heap-allocates a Vec of string pairs to check a boolean. | Add `IS_WATERLOGGED = 1 << 10` to `state_flags` bitfield (currently uses bits 0-9). Single bitwise AND. 10-50× faster, zero alloc. |
| **Palette `.position()` search** | **GAP (bug)** | `palette.rs:125` (pack) and `palette.rs:195` (count). Both use `palette.iter().position()` which is O(palette_len) per lookup. | Pack: build `HashMap<V, usize>` from palette before loop. Count: values ARE palette indices — just index directly into `counts[palette_index]` instead of searching. |
| **Entity `AtomicCell::load()`** | **GAP (design)** | Entity positions stored in `crossbeam::atomic::AtomicCell<Vector3<f64>>`. Each spatial query loads each entity's position atomically. | SoA layout: separate `Vec<f64>` for x, y, z positions. Enables SIMD distance computation. Requires entity position update protocol change. |

---

## C. Planned Modules — Existence Check

From the summary description of what was supposed to be built:

| Module | Status | Notes |
|--------|--------|-------|
| `aabb` (batch AABB intersection w/ SSE4.1) | **MISSING** | No file, no stub |
| `nibble` (4-bit light-level pack/unpack w/ AVX2) | **MISSING** | No file, no stub |
| `property_mask` (compiled bitmask queries w/ AVX2) | **MISSING** | No file, no stub. `state_flags` bitfield exists (u16, 10 bits used) but no AVX2 batch query |
| `spatial_hash` (grid-based spatial index w/ KNN) | **PARTIAL** | `SpatialOverlay` is a fingerprint/Hamming vector, not a spatial index with KNN. Different data structure needed. |
| `clam` (chunk-column claim tracking) | **MISSING** | No file, no stub |
| `crystal_encoder` (run-length block encoding) | **MISSING** | No file, no stub |
| `arrow_bridge` (Arrow IPC serde) | **PARTIAL** | `LanceStore` stub exists with `hydrate_from()` TODO. No Arrow RecordBatch construction code. |
| `byte_scan` (varint + length-prefixed protocol parsing) | **MISSING** | No file, no stub |
| `kernels` (batch dot/saxpy/reduce) | **MISSING** | No file, no stub |
| Jitson JIT engine (Cranelift codegen) | **MISSING** | No Cranelift dependency, no JIT infrastructure |
| Holo optics module | **MISSING** | `holograph.rs` is a 76-line shadow-mode scaffold, not a holographic computation module |
| `palette_codec` (variable-width bit-packed indices) | **MISSING** | `palette.rs` does bit-packing but scalar, no SIMD codec |
| `zeck` (Zeckendorf/Fibonacci encoding) | **MISSING** | No file, no stub |
| `hamming_top_k_raw` | **MISSING** | `SpatialOverlay::hamming_distance()` exists (scalar). No top-k, no batch, no SIMD. |
| `distance` kernels | **MISSING** | Entity distance is `squared_distance_to_vec()` — scalar, no batch |
| Bitwise ops module | **PARTIAL** | `SpatialOverlay` has `xor_diff` + `hamming_weight` (scalar loops). No SIMD dispatch. |

---

## D. Efficiency Assessment — Methods Missing for Real Performance

### D1. No SIMD Infrastructure at All

Zero SIMD anywhere. No `std::arch`, no `wide`, no `pulp`, no `macerator`, no `#[target_feature]` annotations. The `SIMD_COMPARISON.md` recommends `wide` or `pulp` for stable Rust — neither is in `Cargo.toml`.

**What's needed:**
1. Add `wide` or `pulp` to workspace deps
2. Create `pumpkin-util/src/simd/` module with feature-gated dispatch
3. Runtime detection: `is_x86_feature_detected!("avx2")` → dispatch

### D2. No Batch APIs

Every hot path is point-at-a-time:
- `noise::sample(x, y, z) -> f64` — one point
- `get_nearby_entities(pos, radius)` — one query
- `has_random_ticks(state_id)` — one check
- `is_waterlogged()` — one check

**What's needed:**
- `sample_batch(xs: &[f64], ys: &[f64], zs: &[f64], out: &mut [f64])` — N points
- `get_entities_in_cells(cells: &[CellId]) -> Vec<EntityId>` — spatial batch
- `has_random_ticks_batch(ids: &[u16]) -> u64` — batch bitset check (bitmask result)

### D3. No SoA Layout

All game entities are AoS (Array of Structures):
- `Vec<Arc<Player>>` — per-player Arc, per-field atomic loads
- `Vec<Arc<dyn EntityBase>>` — trait objects, vtable dispatch

**What's needed for ARCH-029:**
- `positions_x: Vec<f32>`, `positions_y: Vec<f32>`, `positions_z: Vec<f32>` — SoA
- Or `ndarray::Array2<f32>` with shape `(n_entities, 3)` — columnar
- This is the biggest architectural change and blocks all SIMD entity processing

### D4. No Spatial Index

`SpatialOverlay` is a Hamming fingerprint for *detecting change*, not for *querying neighbors*. The codebase has no:
- Grid-based spatial hash (cell → entity list)
- k-d tree
- BVH (bounding volume hierarchy)
- R-tree

Every spatial query is O(n) over all entities.

---

## E. Summary Scorecard

| Category | Items | Perfect | Solid | Partial | Gap | Missing |
|----------|-------|---------|-------|---------|-----|---------|
| Infrastructure (A) | 4 | 1 | 2 | 1 | 0 | 0 |
| Tier 1 hotspots | 4 | 0 | 0 | 0 | 3 | 1 |
| Tier 2 hotspots | 5 | 0 | 0 | 0 | 3 | 2 |
| Tier 3 hotspots | 5 | 0 | 1 | 0 | 1 | 3 |
| Zero-jitson fixes | 3 | 0 | 0 | 0 | 3 | 0 |
| Planned modules | 16 | 0 | 0 | 3 | 0 | 13 |
| **Total** | **37** | **1** | **3** | **4** | **10** | **19** |

**Bottom line:** The analysis and design documents (`SIMD_COMPARISON.md`, ARCH-029 vision, `SpatialOverlay` + `MobGoalState` primitives, `pumpkin-store` abstraction) are excellent. But 19 of 37 items are completely missing code, and 10 more are gaps where the design exists but no implementation connects it to the hot paths. The one truly complete piece is `MobGoalState`. The three zero-jitson fixes (`is_waterlogged`, palette `.position()`, entity AoS→SoA) are the highest-ROI items because they're plain bugs, not SIMD work.
