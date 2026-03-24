# jitson Shopping List Results

## Tier 1 — Highest Impact

| # | Hotspot | File | Speedup | Why |
|---|---------|------|---------|-----|
| 1 | Palette unpacking | chunk/palette.rs:141-213 | 4-8x | 98K blocks/chunk, every load |
| 5 | Noise octave sampling | noise/perlin.rs:292-307 | 2-4x | 98-131K calls/chunk, CPU-bound worldgen |
| 6 | Entity spatial search | world/mod.rs:2551-2579 | 5-20x | Every tick, O(N) brute force |
| 9 | Block property queries | block_state.rs:134-143 | 10-50x | Box+Vec alloc per call, string search |

## Tier 2 — High Impact

| # | Hotspot | File | Speedup | Why |
|---|---------|------|---------|-----|
| 7 | Entity-player collision | world/mod.rs:728-751 | 4-10x | O(N×M) nested, every tick |
| 8 | Block collision gathering | world/mod.rs:1166-1201 | 3-6x | Per entity movement, 50-200 blocks |
| 4 | Light nibble batch | chunk/format/mod.rs:453-478 | 4-8x | 8192 extractions per section |
| 10 | Random tick counting | generated/block.rs:3314 | 8-16x | VPOPCNTDQ over 64 u64 words |

## Existing Foundation

- `pumpkin-store/src/traits.rs` has SpatialOverlay (256×u64 Hamming vector) — VPOPCNTDQ drop-in
- `pumpkin-data/build/bitsets.rs` generates aligned u64 bitset arrays — SIMD-ready
- Zero SIMD intrinsics in codebase currently
- exp/storage-lance-abtest and claude/compare-simd-implementations branches pruned
