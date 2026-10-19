//! Pure crafting logic for the Path of Exile 1 crafting simulator.
//!
//! This crate has no UI and no I/O. It returns structured results and reason codes,
//! never user-facing text.

/// Version of the `crafting-core` package, for example `"0.1.0"`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_major_minor_patch() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            version()
        );
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
