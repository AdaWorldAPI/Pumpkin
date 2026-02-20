use std::sync::OnceLock;

/// Runtime AI strategy selector.
///
/// This is intentionally additive: default remains `Vanilla`, and optional
/// experimental providers can be selected via `PUMPKIN_AI_PROVIDER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProviderKind {
    Vanilla,
    ExperimentalV1,
    #[cfg(feature = "holograph-provider")]
    Holograph,
}

impl AiProviderKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::ExperimentalV1 => "experimental_v1",
            #[cfg(feature = "holograph-provider")]
            Self::Holograph => "holograph",
        }
    }

    fn from_env(raw: &str) -> Self {
        #[cfg(feature = "holograph-provider")]
        if raw.eq_ignore_ascii_case("holograph") || raw.eq_ignore_ascii_case("holo") {
            return Self::Holograph;
        }

        match raw {
            "experimental" | "experimental_v1" | "exp" => Self::ExperimentalV1,
            _ => Self::Vanilla,
        }
    }
}

/// Returns the process-wide selected AI provider.
///
/// Selection is read once from `PUMPKIN_AI_PROVIDER` and cached.
#[must_use]
pub fn selected_ai_provider() -> AiProviderKind {
    static PROVIDER: OnceLock<AiProviderKind> = OnceLock::new();
    *PROVIDER.get_or_init(|| {
        let provider = std::env::var("PUMPKIN_AI_PROVIDER")
            .ok()
            .map_or(AiProviderKind::Vanilla, |value| {
                AiProviderKind::from_env(value.trim())
            });

        log::info!("Selected AI provider: {}", provider.as_str());
        provider
    })
}
