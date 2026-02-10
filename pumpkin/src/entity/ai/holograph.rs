use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Per-tick decision emitted by the holograph adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HolographTickPlan {
    pub tick_goals_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolographPlanError {
    RuntimeDisabled,
    ForcedFailure,
}

/// Returns whether holograph path should run in shadow mode.
///
/// Shadow mode keeps vanilla as the source of truth and only compares decisions.
/// Default is `true` for safe rollout.
#[must_use]
pub fn holograph_shadow_mode_enabled() -> bool {
    static SHADOW_MODE: OnceLock<bool> = OnceLock::new();
    *SHADOW_MODE.get_or_init(|| {
        std::env::var("PUMPKIN_HOLOGRAPH_SHADOW")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            || std::env::var("PUMPKIN_HOLOGRAPH_SHADOW").is_err()
    })
}

fn holograph_forced_failure() -> bool {
    static FORCE_FAIL: OnceLock<bool> = OnceLock::new();
    *FORCE_FAIL.get_or_init(|| {
        std::env::var("PUMPKIN_HOLOGRAPH_FORCE_FAIL")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

static HOLOGRAPH_RUNTIME_DISABLED: AtomicBool = AtomicBool::new(false);

#[must_use]
pub fn holograph_runtime_enabled() -> bool {
    !HOLOGRAPH_RUNTIME_DISABLED.load(Ordering::Relaxed)
}

pub fn disable_holograph_runtime(error: HolographPlanError) {
    if HOLOGRAPH_RUNTIME_DISABLED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        log::warn!(
            "Holograph AI disabled after error ({error:?}); falling back to vanilla provider"
        );
    }
}

/// Minimal holograph adapter entrypoint.
///
/// This is additive scaffolding: it currently mirrors vanilla branch selection
/// to enable safe shadow-mode rollout and fallback handling before wiring
/// external holograph logic.
pub fn evaluate_holograph_tick_plan(
    age: i32,
    entity_id: i32,
) -> Result<HolographTickPlan, HolographPlanError> {
    if !holograph_runtime_enabled() {
        return Err(HolographPlanError::RuntimeDisabled);
    }

    if holograph_forced_failure() {
        return Err(HolographPlanError::ForcedFailure);
    }

    let tick_goals_only = (age + entity_id) % 2 != 0 && age > 1;
    Ok(HolographTickPlan { tick_goals_only })
}
