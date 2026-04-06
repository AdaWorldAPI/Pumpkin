//! Hardware acceleration layer for Pumpkin.
//!
//! Provides batch processing primitives backed by `ndarray` (`AdaWorldAPI` fork)
//! with multi-tier SIMD dispatch: AMX → AVX-512 → AVX2 → NEON → scalar.
//!
//! # Feature gate
//!
//! All types in this module require the `simd` feature:
//! ```toml
//! pumpkin-util = { path = "..", features = ["simd"] }
//! ```
//!
//! # Architecture
//!
//! - [`batch`] — Batch processing over `&[f64]` slices (noise fill, density).
//! - [`spatial`] — SIMD-accelerated Hamming / XOR / popcount for spatial overlays.
//! - [`hardening`] — Runtime bounds-checking and NaN guards for SIMD outputs.

pub mod batch;
pub mod hardening;
pub mod spatial;
