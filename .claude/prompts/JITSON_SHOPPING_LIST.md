# JITSON Shopping List — Where Pumpkin Benefits from JIT-Compiled JSON

## What jitson does

JSON config values become CPU immediates via Cranelift JIT.
`threshold: 500` → `CMP reg, 500` (not LOAD + CMP).
`focus_mask: [47, 193]` → VPANDQ bitmask as immediate data.
Cold compile: 521µs. Cache hit: 455ns.

Also provides PackedDatabase: stroke-aligned memory layout where data
for the same operation across all candidates is contiguous. Enables
perfect sequential prefetching and 90% early rejection per stroke.

## SIMD primitives available

- Hamming distance (VPOPCNTDQ accelerated)
- Packed 3-stroke cascade (128B/384B/1536B progressive rejection)
- Focus masks (VPANDQ bitmask selects active dimensions)
- Prefetch hints (PREFETCHT0 baked into scan loop)
- Top-K heap with early termination
- VPTERNLOGD (3-input truth table, checks 3 conditions in 1 cycle)

## Where this maps to Minecraft

These are HYPOTHESES. Your job: READ the actual Pumpkin source, find
the real hotspots, and return a shopping list table.

1. **Chunk palette unpacking** — block state IDs packed as bits in longs.
   4096 blocks × 16 sections = 65536 lookups per chunk.

2. **Light propagation** — BFS, 4-bit light levels, XOR/AND/compare.

3. **Collision AABB batching** — entity vs block grid, 16 AABBs at once.

4. **Entity search near position** — currently linear scan?

5. **Block state property matching** — "waterlogged AND facing north?"
   Multiple bit tests → single VPTERNLOGD.

6. **NBT tag scanning** — find all "Entities" tags across 1024 chunks.

7. **Noise generation** — Perlin/simplex parameters as baked immediates.

8. **Tick scheduling** — which blocks need random ticks? Bitmask + VPOPCNTDQ.

## What I need

Read Pumpkin's source. Check the branches `exp/storage-lance-abtest`
and `claude/compare-simd-implementations-btTgj` for existing SIMD work.

Return a table:

| Hotspot | File | Current approach | What jitson would do | Estimated speedup |
|---------|------|-----------------|---------------------|-------------------|

Focus on anything that:
- Loops over 4096+ items
- Unpacks bit-packed data
- Does property matching / filtering
- Searches collections
- Loads parameters from config per iteration

Don't guess. READ the code.
