//! `paint-app` crate. See `docs/Architecture.md`.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        assert_eq!(env!("CARGO_PKG_NAME"), "paint-app");
    }
}
