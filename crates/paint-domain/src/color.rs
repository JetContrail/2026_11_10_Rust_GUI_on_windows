/// An RGBA color with 8 bits per channel (straight, non-premultiplied alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    /// Opaque color from RGB components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub const fn from_array([r, g, b, a]: [u8; 4]) -> Self {
        Self { r, g, b, a }
    }

    /// `#RRGGBB` notation (alpha omitted).
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// The default 16-color palette shown in the tool palette.
pub const PALETTE: [Color; 16] = [
    Color::rgb(0, 0, 0),
    Color::rgb(128, 128, 128),
    Color::rgb(192, 192, 192),
    Color::rgb(255, 255, 255),
    Color::rgb(128, 0, 0),
    Color::rgb(255, 0, 0),
    Color::rgb(255, 128, 0),
    Color::rgb(255, 255, 0),
    Color::rgb(0, 128, 0),
    Color::rgb(0, 255, 0),
    Color::rgb(0, 128, 128),
    Color::rgb(0, 255, 255),
    Color::rgb(0, 0, 128),
    Color::rgb(0, 0, 255),
    Color::rgb(128, 0, 128),
    Color::rgb(255, 0, 255),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_is_opaque() {
        assert_eq!(Color::rgb(1, 2, 3).a, 255);
    }

    #[test]
    fn array_round_trip() {
        let c = Color::rgba(10, 20, 30, 40);
        assert_eq!(Color::from_array(c.to_array()), c);
    }

    #[test]
    fn hex_notation() {
        assert_eq!(Color::rgb(255, 128, 0).to_hex(), "#FF8000");
    }

    #[test]
    fn palette_has_unique_opaque_colors() {
        let unique: std::collections::HashSet<_> = PALETTE.iter().collect();
        assert_eq!(unique.len(), PALETTE.len());
        assert!(PALETTE.iter().all(|c| c.a == 255));
    }
}
