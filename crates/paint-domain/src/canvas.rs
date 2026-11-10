use crate::DomainError;

/// Size of the canvas in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanvasSize {
    pub width: u32,
    pub height: u32,
}

impl CanvasSize {
    /// Maximum width/height. Keeps the memory usage of one RGBA layer at
    /// 64 MiB or less so that the whole application stays below 500 MB.
    pub const MAX_DIMENSION: u32 = 4096;
    pub const DEFAULT: Self = Self {
        width: 1280,
        height: 720,
    };

    /// Creates a validated canvas size (1..=MAX_DIMENSION on both axes).
    pub fn new(width: u32, height: u32) -> Result<Self, DomainError> {
        let valid = |v: u32| (1..=Self::MAX_DIMENSION).contains(&v);
        if valid(width) && valid(height) {
            Ok(Self { width, height })
        } else {
            Err(DomainError::InvalidCanvasSize { width, height })
        }
    }

    pub const fn pixel_count(self) -> usize {
        self.width as usize * self.height as usize
    }
}

impl Default for CanvasSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_sizes() {
        assert!(CanvasSize::new(1, 1).is_ok());
        assert!(CanvasSize::new(4096, 4096).is_ok());
        assert_eq!(CanvasSize::default().pixel_count(), 1280 * 720);
    }

    #[test]
    fn rejects_invalid_sizes() {
        assert!(CanvasSize::new(0, 10).is_err());
        assert!(CanvasSize::new(10, 4097).is_err());
    }
}
