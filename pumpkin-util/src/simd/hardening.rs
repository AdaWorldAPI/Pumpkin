//! Runtime hardening for SIMD outputs.
//!
//! SIMD code can silently produce NaN/Inf from denormals, division-by-zero
//! in vectorised lanes, or out-of-range indices from gather operations.
//! These guards catch invalid outputs before they propagate into world
//! state.
//!
//! All guards are `#[inline]` so they compile away in release builds when
//! the branch is never taken (profile-guided dead-code elimination).

use ndarray::{ArrayView1, Zip};

/// Check that every element in the slice is finite (not NaN, not Inf).
///
/// Returns the index of the first non-finite element, or `None` if all
/// are valid.
///
/// # Use case
///
/// After batch noise fill, call this to detect SIMD lane corruption
/// before writing density values into the chunk buffer.
#[inline]
#[must_use]
pub fn find_non_finite(data: &[f64]) -> Option<usize> {
    data.iter().position(|v| !v.is_finite())
}

/// Clamp all values in `data` to `[min, max]`, replacing NaN with
/// `fallback`.
///
/// This is the "belt and suspenders" guard for density functions -- even
/// if a SIMD lane produces garbage, the output is bounded.
#[inline]
pub fn sanitize_f64(data: &mut [f64], min: f64, max: f64, fallback: f64) {
    let mut view = ndarray::ArrayViewMut1::from(data);
    for v in &mut view {
        if v.is_nan() {
            *v = fallback;
        } else {
            *v = v.clamp(min, max);
        }
    }
}

/// Verify two arrays are element-wise equal within tolerance `eps`.
///
/// Used for regression-testing SIMD batch results against scalar
/// reference implementations. Returns the index of the first divergent
/// element.
#[inline]
#[must_use]
pub fn find_divergence(a: &[f64], b: &[f64], eps: f64) -> Option<usize> {
    assert_eq!(a.len(), b.len(), "find_divergence: length mismatch");
    a.iter()
        .zip(b.iter())
        .position(|(&x, &y)| (x - y).abs() > eps)
}

/// Bounds-check a batch of indices against `max` (exclusive).
///
/// Catches out-of-range permutation table lookups from SIMD gather
/// operations. Returns the index of the first invalid element, or `None`
/// if all are valid.
#[inline]
#[must_use]
pub fn find_out_of_bounds(indices: &[usize], max: usize) -> Option<usize> {
    indices.iter().position(|&i| i >= max)
}

/// Sum an f64 slice using ndarray's 8-fold unrolled accumulation.
///
/// More numerically stable than naive summation for large arrays due to
/// the interleaved accumulators reducing catastrophic cancellation.
#[inline]
#[must_use]
pub fn stable_sum(data: &[f64]) -> f64 {
    let view = ArrayView1::from(data);
    view.sum()
}

/// Dot product using ndarray's unrolled kernel.
///
/// On the `AdaWorldAPI` fork this dispatches to FMA-backed dot product.
#[inline]
#[must_use]
pub fn dot_f64(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "dot_f64: length mismatch");
    let va = ArrayView1::from(a);
    let vb = ArrayView1::from(b);
    Zip::from(&va)
        .and(&vb)
        .fold(0.0, |acc, &a_val, &b_val| a_val.mul_add(b_val, acc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_detection() {
        assert!(find_non_finite(&[1.0, 2.0, 3.0]).is_none());
        assert_eq!(find_non_finite(&[1.0, f64::NAN, 3.0]), Some(1));
        assert_eq!(find_non_finite(&[f64::INFINITY]), Some(0));
        assert_eq!(find_non_finite(&[f64::NEG_INFINITY]), Some(0));
    }

    #[test]
    fn sanitize() {
        let mut data = [f64::NAN, -100.0, 0.5, 100.0, f64::INFINITY];
        sanitize_f64(&mut data, -1.0, 1.0, 0.0);
        assert!((data[0] - 0.0).abs() < f64::EPSILON); // NaN -> fallback
        assert!((data[1] - (-1.0)).abs() < f64::EPSILON); // clamped
        assert!((data[2] - 0.5).abs() < f64::EPSILON); // unchanged
        assert!((data[3] - 1.0).abs() < f64::EPSILON); // clamped
        assert!((data[4] - 1.0).abs() < f64::EPSILON); // Inf clamped
    }

    #[test]
    fn divergence_detection() {
        let a = [1.0, 2.0, 3.0];
        let b = [1.0, 2.0, 3.0];
        assert!(find_divergence(&a, &b, 1e-10).is_none());

        let c = [1.0, 2.1, 3.0];
        assert_eq!(find_divergence(&a, &c, 0.01), Some(1));
    }

    #[test]
    fn out_of_bounds_detection() {
        assert!(find_out_of_bounds(&[0, 1, 255], 256).is_none());
        assert_eq!(find_out_of_bounds(&[0, 256, 1], 256), Some(1));
    }

    #[test]
    fn stable_sum_basic() {
        let data = [1.0, 2.0, 3.0, 4.0];
        assert!((stable_sum(&data) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dot_product() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        // 1*4 + 2*5 + 3*6 = 32
        assert!((dot_f64(&a, &b) - 32.0).abs() < f64::EPSILON);
    }
}
