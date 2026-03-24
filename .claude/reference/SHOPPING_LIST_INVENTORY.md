# Shopping List Inventory — Codebase Reality Check

Cross-referenced 3-session consolidated shopping list against actual Pumpkin source.
Date: 2026-03-24. Branch: claude/worldgen-terrain-biomes-P3zSp (rebased on master).

## Rating Key

- **PERFECT** — Shopping list matches code exactly, speedup estimate realistic
- **ACCURATE** — Correct diagnosis, minor caveats
- **OVERSTATED** — Real hotspot but speedup estimate too high
- **WRONG TARGET** — Code exists but optimizing it won't help for stated reason
- **NOT IMPLEMENTED** — Target code doesn't exist yet
- **DUPLICATE** — Same optimization as another item

---

## TIER 1

### #1 Terrain fill loop — `proto_chunk.rs:677-730`
**Rating: OVERSTATED (1.3-2× realistic, not 2-4×)**

The 6-deep nested loop exists. But the inner body is `interpolate_z()` + `sample_block_state()` —
these are **trilinear interpolation between pre-computed cell corners**, not raw noise.
The expensive density sampling happens at cell boundaries (`sample_start_density`,
`sample_end_density`), not per-block. Per-block cost is lerp3 (3 FMA ops) + aquifer lookup.

- **What's perfect:** Loop structure, 98K blocks/chunk, file location
- **Gap:** The lerps are already FMA-friendly. SIMD gain requires batching 16 Z-positions per
  cell row. Restructuring `interpolate_z` to process a `[f64; 16]` would help but the aquifer
  check is branchy and data-dependent
- **Missing method:** No `sample_block_state_batch(z_range)`. Would need API change to
  `ChunkNoiseGenerator` to expose vectorized interpolation

### #2 Entity spatial search — `world/mod.rs:2483-2649`
**Rating: PERFECT**

Linear scan confirmed. 6 functions, all O(n):
- `get_entities_at_box` (line 2483) — Vec alloc
- `get_players_at_box` (line 2491) — Vec alloc
- `get_nearby_players` (line 2551) — Vec alloc, squared distance
- `get_nearby_entities` (line 2564) — **HashMap alloc** (wasteful)
- `get_closest_player` (line 2582) — calls get_nearby_players → Vec alloc, then min_by
- `get_closest_entity` (line 2615) — calls get_nearby_entities → HashMap, optional filter
  into **second HashMap**, then min_by

**Bugs found:**
- `get_closest_entity` allocates 2 HashMaps for a single-entity result
- `get_closest_player` reads `.pos.load()` twice per comparison in min_by (should cache)
- No chunk-based entity grouping exists anywhere in the codebase

**SpatialOverlay (pumpkin-store) is NOT a spatial index.** It's a 16K-bit Hamming sketch for
activity detection. Cannot answer "which entities near X?" — only "did something change?"

### #3 Palette unpack + count — `palette.rs:141-213`
**Rating: ACCURATE (3-8× for bit extraction, ∞× for count fix)**

Two separate hotspots in one function:

**Hotspot A — Bit extraction (lines 167-186):** 4096 iterations, one i64 word at a time,
shift + mask per entry. SIMD can process 4-8 words in parallel with VPSHUFB/VPERMD.
Realistic speedup: 3-6× for the extraction loop alone.

**Hotspot B — Count building (lines 192-200):** `palette.iter().position()` per block =
O(4096 × palette_size). With 256-entry palette this is ~1M comparisons. **This is the real
perf bug.** Fix: build a `[u16; palette_size]` count array indexed by palette index during
extraction (lines 167-186), eliminating the second pass entirely. Cost: zero. Speedup: 10-50×
on the count path.

**Missing methods:**
- No `from_palette_and_packed_data_simd()` for batch extraction
- No direct-index count accumulation during extraction
- Packing also has `.position()` bug at `palette.rs:108-138` (pack path, `to_palette_and_packed_data`)

### #4 Entity-player collision — `world/mod.rs:728-751`
**Rating: DUPLICATE of #2**

O(entities × players) nested loop confirmed. But this is **not an independent optimization
target**. If #2 adds a spatial index (grid/quadtree), entity-player collision becomes a
spatial query on the same index. Shopping list treats these as separate items — they share
the same data structure fix.

The `break` after first collision means real-world cost is O(entities × avg_nearby_players),
not O(E×P).

---

## TIER 2

### #5 Projectile ray-AABB — `projectile/mod.rs:96-270`
**Rating: ACCURATE (2-4× realistic, not 4-8×)**

`calculate_ray_intersection` (line 243-277) is a classic slab test — 3-axis loop, 6 divisions.
Called per block collision box (line 146-167) and per entity candidate (line 171-188).

- **What's perfect:** Algorithm, location, per-tick cost (50-200 AABBs)
- **Gap:** The async `get_block_collisions` dominates latency. Ray-AABB test is ~20 FLOPs
  per box × 200 boxes = 4000 FLOPs — too small for SIMD to matter unless batched
- **Missing method:** No `calculate_ray_intersection_batch(ray, &[BoundingBox])`. Would need
  to collect all BBs first, then test in SIMD lanes (4 AABBs per 256-bit register)

### #6 Block collision gather — `world/mod.rs:1132-1201`
**Rating: ACCURATE but async-bound**

`get_block_collisions` iterates block positions, calls `self.get_block_state(&pos).await`
per block. The `check_collision` function is pure and fast (iterator over static collision
shapes + AABB intersect). The bottleneck is the **async block state lookup**, not the math.

- **What's perfect:** Code structure, algorithm
- **Wrong target:** SIMD on the intersection math saves <10% when 90% is async chunk lookup
- **Real fix:** Batch block state reads. `get_block_states_batch(&[BlockPos])` that loads
  an entire chunk section once, then returns all states. This is a data access pattern fix,
  not a SIMD opportunity

### #7 Palette pack — `palette.rs:369-414` (Java), `417-480` (Bedrock)
**Rating: ACCURATE (3-6×)**

Both Java and Bedrock packing use `palette.iter().position()` per block — same O(n) linear
search bug as unpacking.

- **Java path (line 381):** `data.palette.iter().position(|&x| x == *key)` inside
  `chunks().map().fold()` — O(4096 × palette_size)
- **Bedrock path (line 458):** Uses `HashMap` for key→index — **already fixed for Bedrock!**
  Java path is slower than Bedrock path for the same data
- **Fix:** Build HashMap once (like Bedrock does) for Java path. Then SIMD for the bit packing

### #8 Light nibble batch extract — `format/mod.rs:411-478`
**Rating: ACCURATE for batch, but no batch callers exist**

`LightContainer::get(x,y,z)` extracts one nibble at a time. Batch extraction of 4096 nibbles
per section would benefit from SIMD (AVX2: 32 nibbles per VPSHUFB).

- **What's perfect:** Data structure, nibble layout, 2048 bytes per section
- **Gap:** No code currently calls `get()` in a batch loop. Light data is read from disk
  (Anvil format) and sent to clients as raw bytes. The nibble accessor is used for
  individual block queries only
- **Not implemented:** No light propagation algorithm exists. When BFS is added, batch
  nibble read/write becomes critical. Until then, this is speculative

### #9 Vicinity tick distance — `world/mod.rs:250-306`
**Rating: OVERSTATED (1.5-2× realistic, not 3-6×)**

`should_tick_entity_vicinity` iterates players to find nearest one, computing squared
distance per player. This is O(players) per entity per tick.

- **What's perfect:** Location, algorithm
- **Gap:** Player count is typically 20-100. The squared distance check is 9 arithmetic ops.
  Total: 900-9000 ops per entity — too small for SIMD to shine
- **Real fix:** Same spatial index as #2. Once entities and players are in a grid, vicinity
  check becomes O(1) grid cell lookup, not O(players) scan

---

## TIER 3

### #10 Noise octave sampling — `perlin.rs:292-307`
**Rating: OVERSTATED (1.2-1.5× realistic, not 1.5-3×)**

Octave loop confirmed: `samplers.iter().map(|data| { ... }).sum()`. Each octave calls
`sample_no_fade` which does 8 permutation lookups + 8 gradient dot products + lerp3.

- **What's perfect:** Algorithm, call count per chunk
- **Why speedup is low:**
  1. Permutation table lookups (`self.permutation[(input & 0xFF)]`) are **random 256-byte
     table gathers** — these are the bottleneck, and they resist SIMD vectorization
  2. Parameters (amplitude, persistence, lacunarity) are already cached in struct fields
  3. The gradient dot product uses `mul_add` which compiles to FMA already
  4. `maintain_precision` (line 176) adds overhead but is necessary for correctness
- **SIMD opportunity:** Process 4 noise samples in parallel (4 different XYZ positions).
  This requires restructuring the caller to provide 4 positions at once — major API change

### #11 Random tick batch count — `bitsets.rs` + `block.rs`
**Rating: ACCURATE for batch, but batch path doesn't exist**

Single `has_random_ticks(id)` is already O(1): array index + bit shift, `#[inline(always)]`.
The benchmark at `pumpkin-data/benches/has_random_ticks.rs` confirms this.

- **What's perfect:** Bitset implementation, O(1) lookup
- **Gap:** The "8-16× batch" claim assumes counting all random-tick blocks in a chunk
  section at once (VPOPCNTDQ over 64 u64 words). But no code does this — random ticks
  pick 3 random blocks per section, not "count all random-tick blocks"
- **Not the right optimization:** The actual random tick path (`level.rs:431-484`) picks
  3 random positions and checks each individually. Batch counting is useful only for
  analytics/monitoring, not gameplay

### #12 NBT batch scan — `tag.rs`
**Rating: ACCURATE (I/O-dominated)**

All 3 sessions agree the CPU portion is 2-4× but I/O dominates. No further analysis needed.

### #13 EndIsland density — `misc.rs:47-62`
**Rating: NICHE, correct**

End-only, confirmed. Low priority.

### #14 Light BFS propagation
**Rating: NOT IMPLEMENTED**

No propagation algorithm exists. Light data is loaded from disk or initialized to defaults
(skylight=15, blocklight=0). S3's VPSUBB batch decay design is valid but there's nothing
to accelerate yet.

---

## ZERO-JITSON FIXES (code bugs, not SIMD targets)

### `is_waterlogged()` — `block_state.rs:134-143`
**Rating: CONFIRMED BUG**

```rust
pub fn is_waterlogged(&self) -> bool {
    let block = Block::from_state_id(self.id);
    block.properties(self.id).is_some_and(|props| {
        props.to_props().iter().any(|(k, v)| k == &"waterlogged" && v == &"true")
    })
}
```

Allocates `Vec<(&str, &str)>` via `to_props()`, iterates with string comparison, for a
boolean check. Fix: add `IS_WATERLOGGED = 1 << 10` to `state_flags`. Build-time cost: zero.
Runtime cost: single bitwise AND. Speedup: 50-100×.

### Palette `.position()` linear search
**Rating: CONFIRMED BUG**

Java pack path at `palette.rs:381` uses `palette.iter().position()` per block.
Bedrock pack at `palette.rs:440` correctly uses `HashMap`. Java path is inconsistent.

### Entity `AtomicCell::load()` per-comparison
**Rating: CONFIRMED INEFFICIENCY**

`get_closest_player` loads `pos` twice per min_by comparison (once for each side of the
comparison). Should load once, cache in a local.

---

## WHAT'S NOT ON THE SHOPPING LIST BUT SHOULD BE

### A. Chunk section `get_block_state` async overhead
**Location:** `world/mod.rs` — called by `get_block_collisions`, entity tick, block updates
**Problem:** Every block state read goes through async chunk lookup. For collision gathering
(50-200 blocks), this means 50-200 async operations for what should be a direct array index
into a loaded chunk section.
**Fix:** `ChunkSection::get_block_state_direct(x, y, z) -> &BlockState` bypassing async.
Chunks are already loaded in memory — the async is for the chunk *loading*, not the lookup.

### B. `compute_collision_math` position tracking — `entity/mod.rs:401-489`
**Location:** `entity/mod.rs:424-440`
**Problem:** The `block_positions` tracking uses a paired iterator that linearly walks
`(collisions_len, position)` tuples to map collision box index → block position. This is
O(blocks) per collision check.
**Fix:** Store block position alongside each BoundingBox in a single Vec<(BoundingBox, BlockPos)>
instead of two parallel Vecs with cumulative-length encoding.

### C. No `EntityBase` downcast cache
**Problem:** `entity.get_entity()` is called 2-5 times per entity per tick across different
methods (spatial search, collision, tick, vicinity). Each call goes through `dyn EntityBase`
vtable dispatch.
**Fix:** Not critical, but a SoA layout where positions are contiguous `[Vector3<f64>; N]`
would make spatial search SIMD-friendly.

---

## SUMMARY MATRIX

| # | Item | Shopping List | Reality | Gap |
|---|------|-------------|---------|-----|
| 1 | Terrain fill | 2-4× | 1.3-2× | Inner loop is lerp, not noise |
| 2 | Entity search | 5-20× | 5-20× | **Perfect** |
| 3 | Palette unpack | 3-8× | 3-6× extract, ∞ count fix | Count bug is the real win |
| 4 | Entity-player | 4-10× | DUPLICATE of #2 | Same spatial index fixes both |
| 5 | Projectile ray | 4-8× | 2-4× | Async dominates, ray math is cheap |
| 6 | Block collision | 2-4× | <10% SIMD gain | Async block lookup is 90% of cost |
| 7 | Palette pack | 3-6× | 3-6× Java, already fixed Bedrock | Java path inconsistent with Bedrock |
| 8 | Light batch | 4-8× | Speculative | No batch callers exist yet |
| 9 | Vicinity tick | 3-6× | 1.5-2× | Too few players for SIMD |
| 10 | Noise octave | 1.5-3× | 1.2-1.5× | Perm table gathers resist SIMD |
| 11 | Random tick | 8-16× batch | No batch path exists | 3 random picks, not batch count |
| 12 | NBT scan | 2-4× CPU | I/O-dominated | Correct assessment |
| 13 | EndIsland | 3-5× | Niche | Correct |
| 14 | Light BFS | 3-8× | NOT IMPLEMENTED | Nothing to optimize yet |

## ACTUAL PRIORITY ORDER (by real-world impact)

1. **Fix is_waterlogged() bug** — free 50-100×, zero SIMD needed
2. **Fix palette count-build O(n²)** — free 10-50×, zero SIMD needed
3. **Fix Java palette pack .position()** — free 5-10×, match Bedrock path
4. **Add spatial index for entities** (#2 + #4 + #9 combined) — 5-20×
5. **Palette unpack SIMD** — 3-6× on hot I/O path
6. **Batch block state reads** (not on list) — removes async overhead from collision
7. **Terrain fill vectorization** — 1.3-2× on worldgen
8. **Projectile ray batch** — 2-4× when many projectiles active

Items 1-3 are **plain bug fixes** that require no SIMD infrastructure at all.
Item 4 is a **data structure change** (spatial hash grid), not a SIMD problem.
Only items 5-8 are genuine SIMD/jitson opportunities.

---

## VOXEL ENGINE MODULES STATUS (from task summary)

| Module | Exists in Repo? | Notes |
|--------|----------------|-------|
| aabb (batch SSE4.1) | NO | BoundingBox exists but scalar only |
| nibble (AVX2 pack/unpack) | NO | LightContainer exists but scalar only |
| property_mask (AVX2 bitmask) | NO | state_flags bitflags exist but scalar |
| spatial_hash (grid KNN) | NO | SpatialOverlay is NOT a spatial index |
| clam (chunk claim tracking) | NO | |
| crystal_encoder (RLE blocks) | NO | |
| arrow_bridge (Arrow IPC) | NO | LanceStore exists but unused |
| byte_scan (varint parsing) | NO | VarInt codec exists but scalar |
| kernels (dot/saxpy/reduce) | NO | |
| jitson (Cranelift JIT) | NO | Only planning docs exist |
| holo (phase/focus/carrier) | NO | SpatialOverlay is the closest thing |
| palette_codec (bit-packed) | NO | palette.rs is scalar |
| zeck (Fibonacci encoding) | NO | |
| hamming_top_k_raw | NO | hamming_distance exists on SpatialOverlay |
| distance kernels | NO | squared_distance_to_vec exists but scalar |
| bitwise ops | NO | bitset generator exists but scalar |

**None of the 9 voxel engine modules exist as code.** The shopping list describes planned
work, not existing implementations. The only SIMD-adjacent code in the repo is:
- `SpatialOverlay` (16K-bit Hamming vector with scalar popcount)
- `MobGoalState` (u64 bit-packed goal encoding with scalar XOR/popcount)
- `gen_u16_bitset` (build-time bitset generator, scalar lookup)
- LLVM auto-vectorization of noise FMA chains (no explicit SIMD)
