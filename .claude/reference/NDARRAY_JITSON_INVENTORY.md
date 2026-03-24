# ndarray/jitson Inventory vs Shopping List

Cross-reference of the 13-hotspot jitson shopping list against the ndarray SIMD
comparison analysis and the described (but not yet landed) voxel engine modules.

## Legend

- **PERFECT**: Module directly addresses the hotspot with correct approach
- **GOOD**: Right idea, needs minor adjustments to match Pumpkin's actual data layout
- **GAP**: Partially covered — key aspects missing or wrong assumptions
- **MESSY**: Code exists but wrong abstraction, wrong data types, or incomplete API
- **MISSING**: Not covered at all

---

## Tier 1 Hotspots

### #1 Palette Unpacking — `palette_codec` module
**Verdict: GAP**

| Aspect | Status | Detail |
|--------|--------|--------|
| Variable bit-width extraction | GOOD | `palette_codec` handles variable-width packed indices |
| Java i64 word format | GAP | Pumpkin packs into `i64` longs, not u64. Sign bit matters for the top entry in each word. Module must handle `i64 as u64` cast correctly |
| Bedrock u32 word format | MISSING | `palette.rs:417-480` packs into `u32` words with `(x,z,y)` axis reorder. `palette_codec` only handles 64-bit words |
| Batch unpack 4096 blocks | GOOD | Correct scale — 4096 per section |
| SIMD approach | GAP | VPSHUFB+VPSRLVD is correct for extraction but bits_per_entry varies (1-15). Need specialized kernels per common width (4, 5, 8, 15) rather than one generic path. Width 4 and 8 are byte-aligned and trivially faster |
| Palette index→block state | MISSING | After unpacking indices, need palette table lookup. Current code does `palette.get(idx)` — a gather operation. Module doesn't address this second step |

**What's needed:**
- Add u32 word support for Bedrock
- Specialize for common bit widths (4, 5, 8, 15)
- Chain: unpack indices → palette gather → block state IDs

---

### #5 Noise Octave Sampling — `noise` module in jitson
**Verdict: GOOD, with important gap**

| Aspect | Status | Detail |
|--------|--------|--------|
| Bake octave params as immediates | PERFECT | `lacunarity`, `amplitude`, `persistence` per octave compiled as constants — eliminates struct field loads |
| Batch 4-8 points | GOOD | ndarray analysis correctly identifies this as the key transformation |
| Permutation table gather | GAP | `self.permutation[(input & 0xFF)]` is the bottleneck (~30% of Perlin cost). Module mentions `VPGATHERDD` but doesn't provide the actual gather kernel or workaround for 8-bit table in 32-bit gather slots |
| Fade curve vectorization | GOOD | `t*t*t*(t*(t*6-15)+10)` maps to SIMD FMA chain |
| lerp3 batching | GOOD | 7 lerps per sample → 7 SIMD ops when batched |
| `maintain_precision` | MISSING | Pumpkin calls `maintain_precision(x * lacunarity)` before each octave sample — this is `x - floor(x / 3.3554432E7) * 3.3554432E7`. Needs SIMD floor |

**What's needed:**
- Implement permutation gather kernel (scalar fallback + AVX2 gather)
- Add `maintain_precision` to the SIMD pipeline
- Ensure biome noise (6 dimensions) batches correctly via `MultiNoiseSampler`

---

### #6 Entity Spatial Search — `spatial_hash` module
**Verdict: PERFECT for the approach, GAP on Pumpkin integration**

| Aspect | Status | Detail |
|--------|--------|--------|
| Grid-based spatial index | PERFECT | Replaces O(N) linear scan with O(1) cell lookup + local scan |
| KNN support | PERFECT | `get_nearby_players()` and `get_nearby_entities()` both need radius-based search |
| Stroke-based cascade | GOOD | Coarse grid → distance check maps to the 2-stroke pattern described |
| Pumpkin data model | GAP | Pumpkin stores entities in `ArcSwap<Arc<Vec<Arc<dyn EntityBase>>>>`. Positions are `AtomicCell<Vector3<f64>>` — loaded atomically per entity. PackedDatabase needs to extract positions into contiguous arrays, but the atomic loads defeat zero-copy |
| Dynamic updates | GAP | Entities move every tick. The spatial index must be rebuilt or incrementally updated every 50ms. Module doesn't address update cost |
| Multi-world | MISSING | Pumpkin has multiple `World` instances, each with its own entity list. Index must be per-world |

**What's needed:**
- Position extraction: batch `entity.get_entity().pos.load()` into contiguous f64 arrays
- Incremental update strategy (full rebuild vs. dirty tracking)
- Per-world index instances

---

### #9 Block Property Queries — `property_mask` module
**Verdict: MESSY — fundamentally mismatched with Pumpkin's architecture**

| Aspect | Status | Detail |
|--------|--------|--------|
| Bitmask-based property query | GOOD idea | Correct optimization target — `is_waterlogged()` allocates Box+Vec per call |
| AVX2 VPTERNLOGD for 3 properties | GOOD idea | Would be excellent for compound queries |
| Pumpkin property encoding | **WRONG ASSUMPTION** | Properties are NOT bit-encoded. `state_flags` is only 10 bits (air, solid, burnable, etc.). Variant properties (waterlogged, facing, powered) are encoded as **ordinal combinations in the state ID itself**, recovered via string-based `to_props()`. There is NO bitmask for variant properties |
| Generated property structs | MESSY | pumpkin-data generates 100+ `XxxLikeProperties` structs with enum fields. These are **not** bitmasks — they're named structs with enum variants |

**The real problem:** `is_waterlogged()` does:
1. `Block::from_state_id(id)` — array lookup, O(1)
2. `block.properties(id)` — match on block.id, returns `Box<dyn BlockProperties>`
3. `props.to_props()` — allocates `Vec<(&str, &str)>`
4. `.any(|(k,v)| k == "waterlogged" && v == "true")` — string comparison

**What's actually needed:**
- Generate a `WATERLOGGED_BITSET: [u64; N]` at build time (like `RANDOM_TICKS_BITSET`)
- For each state_id, precompute `is_waterlogged` and pack into the bitset
- Similar bitsets for `facing=north`, `powered=true`, etc.
- Then VPANDQ across bitsets for compound queries
- No need for runtime bitmask compilation — this is a **build-time** problem, not a JIT problem

---

## Tier 2 Hotspots

### #7 Entity-Player Collision — `aabb` module (SSE4.1)
**Verdict: GOOD**

| Aspect | Status | Detail |
|--------|--------|--------|
| Batch AABB intersection | PERFECT | 6 comparisons → SIMD `CMPPS` for 4 pairs at once |
| SSE4.1 target | GOOD | Conservative target, widely available |
| AoS vs SoA | GAP | Pumpkin's `BoundingBox` is AoS (min.x, min.y, min.z, max.x, max.y, max.z = 48 bytes). Module needs AoS→SoA transpose for batching, or work with AoS directly via lane shuffles |
| Atomic bbox load | GAP | `entity.bounding_box.load()` is an atomic operation per entity. Cannot avoid this — entities move. Must batch the loads first, then run SIMD |
| `expand(1.0, 0.5, 1.0)` | MISSING | The actual collision code calls `bounding_box.expand(1.0, 0.5, 1.0).intersects(other)`. The expand step must be fused into the SIMD kernel |

**What's needed:**
- AoS→SoA transpose for batched bbox data
- Fused expand+intersect kernel
- Handle atomic bbox extraction as a pre-pass

---

### #8 Block Collision Gathering — `aabb` module + collision shapes
**Verdict: GAP**

| Aspect | Status | Detail |
|--------|--------|--------|
| Batch AABB from block grid | GOOD idea | 50-200 blocks per movement check |
| Collision shape lookup | GAP | 683 unique shapes stored as AoS `CollisionShape { min: Vector3, max: Vector3 }`. Index is `u16` per block state. Module doesn't handle the index→shape gather |
| Multiple shapes per block | GAP | Some blocks have 2-4 collision shapes (stairs, fences). Module assumes 1:1 block→AABB |
| Air/solid pre-filter | MISSING | `state_flags & IS_SOLID` and `state_flags & IS_AIR` are bit checks. Should be a SIMD bitmask pre-pass before AABB testing |
| Async chunk lookup | **BLOCKER** | `self.get_block_state(&pos).await` is async per block. The entire gathering loop is async. Must batch chunk reads before SIMD processing |

**What's needed:**
- Pre-pass: gather all block states for the candidate region (batch async reads)
- Bitmask filter: `IS_SOLID & !IS_AIR` via VPANDQ on state_flags
- Scatter: map surviving blocks to their collision shape indices
- Batch AABB test on gathered shapes

---

### #4 Light Nibble Batch — `nibble` module (AVX2)
**Verdict: PERFECT for extraction, GAP for propagation**

| Aspect | Status | Detail |
|--------|--------|--------|
| 4-bit pack/unpack | PERFECT | `data[idx >> 1] >> (4 * (idx & 1)) & 0x0F` — this is exactly nibble extraction |
| AVX2 VPSHUFB for 32 nibbles | PERFECT | Correct instruction for parallel nibble unpack |
| Section-scale batch | GOOD | 4096 nibbles per section, 2048 bytes storage |
| Light propagation BFS | **NOT COVERED** | The nibble module handles storage but not the BFS traversal. Propagation needs: batch neighbor lookup (6 directions × N updates), saturation subtract for decay, comparison for update detection |
| Async propagation | **BLOCKER** | Current light BFS is async (`propagate_block_light_decrease/increase`). Each neighbor check does `self.get_block_light_level(&pos).await` (async chunk lookup). Cannot SIMD-vectorize across await points |

**What's needed:**
- Batch light propagation kernel: given a contiguous section, propagate all pending updates without async
- VPSUBB for light decay (subtract opacity, saturate at 0)
- VPCMPUB for finding cells that need further propagation

---

### #10 Random Tick Counting — `hamming_top_k_raw` / bitwise ops
**Verdict: GOOD but overkill**

| Aspect | Status | Detail |
|--------|--------|--------|
| VPOPCNTDQ over bitset | PERFECT | `RANDOM_TICKS_BITSET` is already 464 × u64 words. VPOPCNTDQ counts eligible blocks instantly |
| Existing bitset | PERFECT | Build infrastructure already generates aligned u64 arrays |
| Top-K | MESSY | Random tick doesn't need Top-K. It needs: (1) count eligible blocks in a chunk section palette, (2) randomly select N positions. Top-K heap is wrong abstraction |
| Per-chunk counting | GAP | The bitset covers ALL 29,670 state IDs. Per-chunk, you have a palette of 5-50 unique states. Need to first check palette membership, then count blocks per palette entry × eligible bits |

**What's needed:**
- Per-palette eligible mask: for each palette entry, check bitset → build 1-bit-per-palette-entry mask
- Count blocks per palette entry (already in `PalettedContainer` counts array)
- Multiply mask × counts → total eligible blocks. No SIMD needed for this — it's 5-50 entries.
- VPOPCNTDQ is useful for the SpatialOverlay Hamming distance, not random tick counting

---

## Tier 3 and Below

### #2/#3 Palette Packing — `palette_codec`
**Verdict: GAP**

Same issues as #1 but in reverse (packing not unpacking). Additional problem:
- **Linear search for palette index**: `palette.iter().position(|&x| x == *key)` is O(palette_size) per block. Need a HashMap or sorted lookup. Module doesn't address this.
- Bedrock u32 packing + axis reorder not covered.

### #11 Light BFS Propagation — NOT COVERED
**Verdict: MISSING**

The `nibble` module handles storage only. The BFS propagation algorithm needs:
- Batch queue drain (current: SegQueue pop one-at-a-time)
- Batch neighbor lookup (6 directions × batch size)
- SIMD decay + comparison
- The async architecture is the fundamental blocker — needs sync batch propagation path

### #12 NBT Tag Scanning — `byte_scan` module
**Verdict: GAP**

| Aspect | Status | Detail |
|--------|--------|--------|
| Varint parsing | GOOD | Protocol varint parsing is a real hotspot |
| Length-prefixed scanning | GOOD | NBT strings are length-prefixed |
| Batch tag type scanning | GAP | NBT tags have variable-length payloads. Cannot simply scan for byte patterns — must parse the tree structure to find tag boundaries. `byte_scan` assumes flat protocol frames, not recursive tree structures |
| Region file batch | MISSING | 1024 chunks per region file. The real opportunity is parallel decompression + parallel NBT parse across chunks, not SIMD within a single NBT tree |

### #13 Vicinity Tick Distance — `kernels` module (batch dot/saxpy)
**Verdict: GOOD**

| Aspect | Status | Detail |
|--------|--------|--------|
| Batch distance calculation | GOOD | `squared_distance_to_vec` is 3 subtracts + 3 multiplies + 2 adds. Batch across 4-8 players via FMA |
| Config thresholds as immediates | GOOD | `near_distance_sq`, `mid_distance_sq` loaded from OnceLock — JIT as immediates |
| Early exit | GAP | Current code exits early when `nearest_distance_sq <= near_distance_sq`. SIMD batch loses this. Need `VMOVMSKPD` to check if any lane hit the threshold |

---

## Summary Matrix

| # | Hotspot | Module | Verdict | Key Issue |
|---|---------|--------|---------|-----------|
| **1** | Palette unpack | `palette_codec` | **GAP** | No Bedrock u32, no palette gather, no width specialization |
| **5** | Noise sampling | `noise` (jitson) | **GOOD** | Missing permutation gather + `maintain_precision` |
| **6** | Entity search | `spatial_hash` | **PERFECT** | Needs Pumpkin-specific position extraction |
| **9** | Property queries | `property_mask` | **MESSY** | Wrong assumption: properties are strings, not bits. Need build-time bitsets instead |
| **7** | Entity collision | `aabb` | **GOOD** | Missing expand+intersect fusion, AoS→SoA transpose |
| **8** | Block collision | `aabb` | **GAP** | Async chunk reads blocker, multi-shape per block, no air pre-filter |
| **4** | Light nibbles | `nibble` | **PERFECT** | Storage only — propagation not covered |
| **10** | Random tick count | `hamming` | **MESSY** | Top-K is wrong abstraction; per-palette counting needs different approach |
| **2/3** | Palette pack | `palette_codec` | **GAP** | Linear palette search, no Bedrock u32 |
| **11** | Light BFS | — | **MISSING** | Async architecture blocker, not covered at all |
| **12** | NBT scanning | `byte_scan` | **GAP** | Recursive tree ≠ flat protocol frames |
| **13** | Distance calc | `kernels` | **GOOD** | Missing SIMD early-exit via movemask |

## Modules Not Mapped to Shopping List

| Module | What It Does | Shopping List Coverage |
|--------|-------------|----------------------|
| `clam` | Chunk-column claim tracking | Not in shopping list. Useful for chunk loading coordination but not a SIMD hotspot |
| `crystal_encoder` | Run-length block encoding | Not in shopping list. Could help chunk compression but not a jitson target |
| `arrow_bridge` | Arrow IPC serde | Infrastructure for ARCH-029 Phase 3, not a direct hotspot fix |
| `zeck` | Zeckendorf/Fibonacci encoding + batch/top_k | Interesting encoding but no Pumpkin hotspot uses Fibonacci encoding |
| `holo optics` | Phase/focus/carrier holographic computations | Completely unrelated to Minecraft server hotspots |

## Critical Gaps Not Covered by Any Module

1. **Async→sync batch bridge**: Many hotspots (block collision, light propagation, block state lookup) are async because they cross chunk boundaries. Need a "prefetch region" primitive that batch-loads all needed chunks, then runs sync SIMD over the loaded data.

2. **Build-time bitset generation for variant properties**: `property_mask` assumes runtime bitmask compilation, but properties are static per state ID. Generate `WATERLOGGED_BITSET`, `FACING_NORTH_BITSET`, etc. at build time in `pumpkin-data/build/bitsets.rs` (infrastructure already exists for `RANDOM_TICKS_BITSET`).

3. **Position extraction pass**: Every entity SIMD operation needs positions extracted from `AtomicCell<Vector3<f64>>` into contiguous arrays. This is a common prerequisite for #6, #7, #13 — needs a dedicated `extract_positions()` utility.

4. **Per-biome JIT noise functions**: The jitson `noise` module compiles generic noise, but each biome has different octave counts and amplitude arrays. Need per-biome compiled functions, not one generic function.
