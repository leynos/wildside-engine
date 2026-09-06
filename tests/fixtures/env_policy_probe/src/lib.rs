//! Probes for the environment-access UI test.
//!
//! Each probe is behind its own feature so one Clippy invocation reports one
//! kind of diagnostic. With no feature enabled the crate is empty and lints
//! clean, which is what makes a failure attributable to the probe under test.

/// Read and mutate the environment directly, once per prohibited method.
///
/// Clippy must reject all six calls.
#[cfg(feature = "bare-read")]
#[must_use]
pub fn probe() -> usize {
    let name = "WILDSIDE_ENV_POLICY_PROBE";
    let mut seen = 0;
    seen += usize::from(std::env::var(name).is_ok());
    seen += usize::from(std::env::var_os(name).is_some());
    seen += std::env::vars().count();
    seen += std::env::vars_os().count();
    // SAFETY: the fixture is never executed. It exists only to be linted, and
    // Clippy reports the call before any of this could run.
    unsafe {
        std::env::set_var(name, "probe");
        std::env::remove_var(name);
    }
    seen
}

/// Read the environment under `#[allow]`. Clippy must reject the attribute.
#[cfg(feature = "allow-bypass")]
#[allow(clippy::disallowed_methods)]
#[must_use]
pub fn probe() -> Option<String> {
    std::env::var("WILDSIDE_ENV_POLICY_PROBE").ok()
}

/// Read the environment at a composition root. Clippy must accept this.
#[cfg(feature = "expect-escape")]
#[expect(
    clippy::disallowed_methods,
    reason = "composition root for WILDSIDE_ENV_POLICY_PROBE"
)]
#[must_use]
pub fn probe() -> Option<String> {
    std::env::var("WILDSIDE_ENV_POLICY_PROBE").ok()
}
