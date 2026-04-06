//! SIMD-accelerated spatial operations for overlay bit-vectors.
//!
//! Replaces the scalar `for word in &bits { ... }` loops in `SpatialOverlay`
//! with ndarray-backed batch operations. The `AdaWorldAPI` ndarray fork will
//! dispatch to AVX-512 `vpxorq` + `vpopcntq` when available, falling back to
//! AVX2 / NEON / scalar automatically.

use ndarray::{ArrayView1, Zip};

/// Number of u64 words in a `SpatialOverlay` (256 x u64 = 16384 bits).
pub const OVERLAY_WORDS: usize = 256;

/// XOR two u64 word arrays into `out`.
///
/// On AVX-512 this compiles to 32 `vpxorq zmm` instructions covering the
/// entire 16384-bit overlay in a single pass.
#[inline]
pub fn xor_words(
    a: &[u64; OVERLAY_WORDS],
    b: &[u64; OVERLAY_WORDS],
    out: &mut [u64; OVERLAY_WORDS],
) {
    let va = ArrayView1::from(a.as_slice());
    let vb = ArrayView1::from(b.as_slice());
    let mut vout = ndarray::ArrayViewMut1::from(out.as_mut_slice());
    Zip::from(&mut vout)
        .and(&va)
        .and(&vb)
        .for_each(|o, &wa, &wb| *o = wa ^ wb);
}

/// Popcount (Hamming weight) of a u64 word array.
///
/// With AVX-512 VPOPCNT (Ice Lake+), this is 32 `vpopcntq` instructions.
/// Otherwise falls back to Rust's `count_ones()` which the compiler maps
/// to the best available instruction (POPCNT on x86, CNT on ARM).
#[inline]
#[must_use]
pub fn popcount_words(words: &[u64; OVERLAY_WORDS]) -> u32 {
    let view = ArrayView1::from(words.as_slice());
    view.iter().map(|w| w.count_ones()).sum()
}

/// Hamming distance between two overlays = popcount(a XOR b).
///
/// Fused into a single pass to avoid materialising the XOR intermediate.
#[inline]
#[must_use]
pub fn hamming_distance(a: &[u64; OVERLAY_WORDS], b: &[u64; OVERLAY_WORDS]) -> u32 {
    let va = ArrayView1::from(a.as_slice());
    let vb = ArrayView1::from(b.as_slice());
    Zip::from(&va)
        .and(&vb)
        .fold(0u32, |acc, &wa, &wb| acc + (wa ^ wb).count_ones())
}

/// Bitwise OR-reduce: `out[i] |= src[i]` for each word.
///
/// Useful for merging multiple overlays (e.g., tick N and tick N+1).
#[inline]
pub fn or_accumulate(src: &[u64; OVERLAY_WORDS], out: &mut [u64; OVERLAY_WORDS]) {
    let vs = ArrayView1::from(src.as_slice());
    let mut vo = ndarray::ArrayViewMut1::from(out.as_mut_slice());
    Zip::from(&mut vo).and(&vs).for_each(|o, &s| *o |= s);
}

/// Bitwise AND of two word arrays into `out`.
///
/// Used for intersection queries -- "which spatial buckets are active in
/// BOTH overlays?"
#[inline]
pub fn and_words(
    a: &[u64; OVERLAY_WORDS],
    b: &[u64; OVERLAY_WORDS],
    out: &mut [u64; OVERLAY_WORDS],
) {
    let va = ArrayView1::from(a.as_slice());
    let vb = ArrayView1::from(b.as_slice());
    let mut vout = ndarray::ArrayViewMut1::from(out.as_mut_slice());
    Zip::from(&mut vout)
        .and(&va)
        .and(&vb)
        .for_each(|o, &wa, &wb| *o = wa & wb);
}

/// Check if two overlays are equal (early-exit on first difference).
#[inline]
#[must_use]
pub fn overlays_equal(a: &[u64; OVERLAY_WORDS], b: &[u64; OVERLAY_WORDS]) -> bool {
    // Use ndarray 8-fold unrolled equality check.
    let va = ArrayView1::from(a.as_slice());
    let vb = ArrayView1::from(b.as_slice());
    va == vb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_words_roundtrip() {
        let mut a = [0u64; OVERLAY_WORDS];
        let mut b = [0u64; OVERLAY_WORDS];
        a[0] = 0xFF;
        b[0] = 0x0F;
        a[255] = u64::MAX;
        let mut out = [0u64; OVERLAY_WORDS];
        xor_words(&a, &b, &mut out);
        assert_eq!(out[0], 0xF0);
        assert_eq!(out[255], u64::MAX);
        assert_eq!(out[1], 0);
    }

    #[test]
    fn popcount() {
        let mut words = [0u64; OVERLAY_WORDS];
        words[0] = 0b1111; // 4 bits
        words[1] = 0b11; // 2 bits
        assert_eq!(popcount_words(&words), 6);
    }

    #[test]
    fn hamming_distance_basic() {
        let a = [0u64; OVERLAY_WORDS];
        let mut b = [0u64; OVERLAY_WORDS];
        b[0] = 0b111; // 3 bits differ
        assert_eq!(hamming_distance(&a, &b), 3);
    }

    #[test]
    fn or_accumulate_merge() {
        let mut out = [0u64; OVERLAY_WORDS];
        out[0] = 0b1010;
        let mut src = [0u64; OVERLAY_WORDS];
        src[0] = 0b0101;
        or_accumulate(&src, &mut out);
        assert_eq!(out[0], 0b1111);
    }

    #[test]
    fn and_words_intersect() {
        let mut a = [0u64; OVERLAY_WORDS];
        let mut b = [0u64; OVERLAY_WORDS];
        a[0] = 0b1111;
        b[0] = 0b1010;
        let mut out = [0u64; OVERLAY_WORDS];
        and_words(&a, &b, &mut out);
        assert_eq!(out[0], 0b1010);
    }

    #[test]
    fn overlays_equal_check() {
        let a = [42u64; OVERLAY_WORDS];
        let b = [42u64; OVERLAY_WORDS];
        assert!(overlays_equal(&a, &b));

        let mut c = [42u64; OVERLAY_WORDS];
        c[100] = 0;
        assert!(!overlays_equal(&a, &c));
    }
}
