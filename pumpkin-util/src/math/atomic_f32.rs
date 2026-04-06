use std::sync::atomic::{AtomicU32, Ordering};

/// A thread-safe atomic wrapper around `f32` values.
pub struct AtomicF32 {
    /// The underlying atomic storage of the float as `u32` bits.
    storage: AtomicU32,
}

impl AtomicF32 {
    /// Creates a new `AtomicF32` initialized with `value`.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        let as_u32 = value.to_bits();
        Self {
            storage: AtomicU32::new(as_u32),
        }
    }

    /// Stores a new value into the atomic float.
    pub fn store(&self, value: f32, ordering: Ordering) {
        let as_u32 = value.to_bits();
        self.storage.store(as_u32, ordering);
    }

    /// Loads the current value of the atomic float.
    pub fn load(&self, ordering: Ordering) -> f32 {
        let as_u32 = self.storage.load(ordering);
        f32::from_bits(as_u32)
    }

    /// Performs a compare-and-exchange operation on the atomic float.
    pub fn compare_exchange(
        &self,
        current: f32,
        new: f32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<f32, f32> {
        let current_bits = current.to_bits();
        let new_bits = new.to_bits();
        self.storage
            .compare_exchange(current_bits, new_bits, success, failure)
            .map(f32::from_bits)
            .map_err(f32::from_bits)
    }
}
