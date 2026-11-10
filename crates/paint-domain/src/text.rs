/// Font size of text operations in canvas pixels (any integer value in
/// [`TextSize::MIN`]..=[`TextSize::MAX`]; fonts are scalable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextSize(u16);

impl TextSize {
    pub const MIN: u16 = 8;
    pub const MAX: u16 = 400;
    /// Common sizes offered as presets next to the numeric input.
    pub const PRESETS: [u16; 7] = [12, 16, 24, 32, 48, 64, 96];

    /// Returns `None` when `px` is outside the supported range.
    pub fn new(px: u16) -> Option<Self> {
        (Self::MIN..=Self::MAX).contains(&px).then_some(Self(px))
    }

    /// Clamps `px` into the supported range.
    pub fn clamped(px: u16) -> Self {
        Self(px.clamp(Self::MIN, Self::MAX))
    }

    pub const fn px(self) -> u16 {
        self.0
    }
}

impl Default for TextSize {
    fn default() -> Self {
        Self(32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_range_is_enforced() {
        assert_eq!(TextSize::new(7), None);
        assert_eq!(TextSize::new(8).map(TextSize::px), Some(8));
        assert_eq!(TextSize::new(400).map(TextSize::px), Some(400));
        assert_eq!(TextSize::new(401), None);
        assert_eq!(TextSize::clamped(0).px(), 8);
        assert_eq!(TextSize::clamped(999).px(), 400);
        assert!(
            TextSize::PRESETS
                .iter()
                .all(|&p| TextSize::new(p).is_some())
        );
    }
}
