/// Selectable line widths / eraser sizes in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum StrokeWidth {
    Px1,
    Px2,
    #[default]
    Px4,
    Px8,
    Px16,
    Px32,
}

impl StrokeWidth {
    pub const ALL: [Self; 6] = [
        Self::Px1,
        Self::Px2,
        Self::Px4,
        Self::Px8,
        Self::Px16,
        Self::Px32,
    ];

    pub const fn px(self) -> u32 {
        match self {
            Self::Px1 => 1,
            Self::Px2 => 2,
            Self::Px4 => 4,
            Self::Px8 => 8,
            Self::Px16 => 16,
            Self::Px32 => 32,
        }
    }

    /// Returns the width for an exact pixel value, `None` if not selectable.
    pub fn from_px(px: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.px() == px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_required_widths_are_available() {
        let px: Vec<u32> = StrokeWidth::ALL.iter().map(|w| w.px()).collect();
        assert_eq!(px, vec![1, 2, 4, 8, 16, 32]);
    }

    #[test]
    fn from_px_round_trip() {
        for w in StrokeWidth::ALL {
            assert_eq!(StrokeWidth::from_px(w.px()), Some(w));
        }
        assert_eq!(StrokeWidth::from_px(3), None);
    }
}
