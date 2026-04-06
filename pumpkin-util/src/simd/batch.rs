//! Batch processing primitives using ndarray.
//!
//! These replace point-at-a-time hot loops in worldgen and density functions
//! with vectorised operations that ndarray's layout-aware iteration can auto-
//! vectorise (or dispatch to explicit SIMD kernels in the `AdaWorldAPI` fork).

use ndarray::{Array1, ArrayView1, Zip};

/// Fill `output` with the result of applying `f` to every element of `input`.
///
/// ndarray's `Zip` detects contiguous layout and emits SIMD-friendly loops.
#[inline]
pub fn batch_map_f64(input: &[f64], output: &mut [f64], f: impl Fn(f64) -> f64) {
    assert_eq!(
        input.len(),
        output.len(),
        "batch_map_f64: input/output length mismatch"
    );
    let src = ArrayView1::from(input);
    let mut dst = ndarray::ArrayViewMut1::from(output);
    Zip::from(&mut dst).and(&src).for_each(|d, &s| *d = f(s));
}

/// Element-wise fused multiply-add: `out[i] = a[i] * b[i] + c[i]`.
///
/// Maps directly to FMA instructions when the CPU supports them.
#[inline]
pub fn batch_fma(a: &[f64], b: &[f64], c: &[f64], out: &mut [f64]) {
    let len = a.len();
    assert!(
        b.len() == len && c.len() == len && out.len() == len,
        "batch_fma: length mismatch"
    );
    let va = ArrayView1::from(a);
    let vb = ArrayView1::from(b);
    let vc = ArrayView1::from(c);
    let mut vout = ndarray::ArrayViewMut1::from(out);
    Zip::from(&mut vout)
        .and(&va)
        .and(&vb)
        .and(&vc)
        .for_each(|o, &a_val, &b_val, &c_val| {
            *o = a_val.mul_add(b_val, c_val);
        });
}

/// Batch linear interpolation: `out[i] = a[i] + t[i] * (b[i] - a[i])`.
#[inline]
pub fn batch_lerp(t: &[f64], a: &[f64], b: &[f64], out: &mut [f64]) {
    let len = t.len();
    assert!(
        a.len() == len && b.len() == len && out.len() == len,
        "batch_lerp: length mismatch"
    );
    let vt = ArrayView1::from(t);
    let va = ArrayView1::from(a);
    let vb = ArrayView1::from(b);
    let mut vout = ndarray::ArrayViewMut1::from(out);
    Zip::from(&mut vout)
        .and(&vt)
        .and(&va)
        .and(&vb)
        .for_each(|o, &t_val, &a_val, &b_val| {
            *o = (b_val - a_val).mul_add(t_val, a_val);
        });
}

/// Batch Perlin fade: `out[i] = t^3 * (t * (t*6 - 15) + 10)`.
///
/// The fade curve is ~40% of Perlin noise cost. Batching N points through
/// this avoids per-point function-call overhead and enables SIMD multiply
/// chains.
#[inline]
pub fn batch_perlin_fade(input: &[f64], output: &mut [f64]) {
    batch_map_f64(input, output, |t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
}

/// Batch squared-distance:
/// `out[i] = (ax[i]-bx[i])^2 + (ay[i]-by[i])^2 + (az[i]-bz[i])^2`.
///
/// For entity spatial queries -- replaces `O(N)` scalar loops with a
/// vectorised pass over structure-of-arrays position columns.
#[inline]
pub fn batch_squared_distance_3d(
    ax: &[f64],
    ay: &[f64],
    az: &[f64],
    bx: &[f64],
    by: &[f64],
    bz: &[f64],
    out: &mut [f64],
) {
    let len = ax.len();
    assert!(
        ay.len() == len
            && az.len() == len
            && bx.len() == len
            && by.len() == len
            && bz.len() == len
            && out.len() == len,
        "batch_squared_distance_3d: length mismatch"
    );
    let vax = ArrayView1::from(ax);
    let vay = ArrayView1::from(ay);
    let vaz = ArrayView1::from(az);
    let vbx = ArrayView1::from(bx);
    let vby = ArrayView1::from(by);
    let vbz = ArrayView1::from(bz);
    let mut vout = ndarray::ArrayViewMut1::from(out);
    // ndarray Zip over 7 arrays -- contiguous layout => SIMD-friendly
    Zip::from(&mut vout)
        .and(&vax)
        .and(&vay)
        .and(&vaz)
        .and(&vbx)
        .and(&vby)
        .and(&vbz)
        .for_each(|o, &ax_v, &ay_v, &az_v, &bx_v, &by_v, &bz_v| {
            let dx = ax_v - bx_v;
            let dy = ay_v - by_v;
            let dz = az_v - bz_v;
            *o = dx.mul_add(dx, dy.mul_add(dy, dz * dz));
        });
}

/// Accumulate octave noise into a pre-allocated output array.
///
/// `octave_fn(lacunarity, scratch)` is called once per octave; `out` is
/// accumulated in-place with `out[i] += amplitude * persistence * sample`.
///
/// This replaces the point-at-a-time `OctavePerlinNoiseSampler::sample`
/// pattern with a batch-first approach that keeps the hot data in SIMD
/// registers across all points.
pub fn batch_octave_accumulate(
    n: usize,
    octaves: &[(f64, f64, f64)], // (lacunarity, persistence, amplitude)
    mut octave_fn: impl FnMut(f64, &mut [f64]),
    out: &mut [f64],
) {
    for v in &mut *out {
        *v = 0.0;
    }
    let mut scratch = vec![0.0f64; n];

    for &(lacunarity, persistence, amplitude) in octaves {
        if amplitude == 0.0 {
            continue;
        }
        octave_fn(lacunarity, &mut scratch);

        let scale = amplitude * persistence;
        let scratch_view = ArrayView1::from(scratch.as_slice());
        let mut out_view = ndarray::ArrayViewMut1::from(&mut *out);
        Zip::from(&mut out_view)
            .and(&scratch_view)
            .for_each(|o, &s| *o += scale * s);
    }
}

/// Create a contiguous `Array1<f64>` from a slice (zero-copy if aligned).
#[inline]
#[must_use]
pub fn to_array1(data: &[f64]) -> ArrayView1<'_, f64> {
    ArrayView1::from(data)
}

/// Allocate a zeroed `Array1<f64>` of length `n`.
#[inline]
#[must_use]
pub fn zeros(n: usize) -> Array1<f64> {
    Array1::zeros(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fma() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let c = [0.1, 0.2, 0.3, 0.4];
        let mut out = [0.0; 4];
        batch_fma(&a, &b, &c, &mut out);
        for i in 0..4 {
            assert!((out[i] - a[i].mul_add(b[i], c[i])).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn lerp() {
        let t = [0.0, 0.25, 0.5, 1.0];
        let a = [10.0, 10.0, 10.0, 10.0];
        let b = [20.0, 20.0, 20.0, 20.0];
        let mut out = [0.0; 4];
        batch_lerp(&t, &a, &b, &mut out);
        let expected = [10.0, 12.5, 15.0, 20.0];
        for i in 0..4 {
            assert!((out[i] - expected[i]).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn perlin_fade() {
        let input = [0.0, 0.5, 1.0];
        let mut output = [0.0; 3];
        batch_perlin_fade(&input, &mut output);
        // fade(0) = 0, fade(0.5) = 0.5, fade(1) = 1
        assert!((output[0]).abs() < f64::EPSILON);
        assert!((output[1] - 0.5).abs() < 1e-10);
        assert!((output[2] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn squared_distance() {
        let ax = [0.0, 1.0];
        let ay = [0.0, 0.0];
        let az = [0.0, 0.0];
        let bx = [3.0, 4.0];
        let by = [4.0, 0.0];
        let bz = [0.0, 0.0];
        let mut out = [0.0; 2];
        batch_squared_distance_3d(&ax, &ay, &az, &bx, &by, &bz, &mut out);
        assert!((out[0] - 25.0).abs() < f64::EPSILON); // 3^2 + 4^2
        assert!((out[1] - 9.0).abs() < f64::EPSILON); // 3^2
    }

    #[test]
    fn octave_accumulate() {
        let octaves = [(1.0, 0.5, 2.0), (2.0, 0.25, 1.0)];
        let mut out = [0.0; 4];
        batch_octave_accumulate(
            4,
            &octaves,
            |lacunarity, scratch| {
                // Simple mock: each sample = lacunarity (constant)
                for v in scratch.iter_mut() {
                    *v = lacunarity;
                }
            },
            &mut out,
        );
        // octave 0: 2.0 * 0.5 * 1.0 = 1.0
        // octave 1: 1.0 * 0.25 * 2.0 = 0.5
        // total = 1.5
        for &v in &out {
            assert!((v - 1.5).abs() < f64::EPSILON);
        }
    }
}
