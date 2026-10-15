//! Colors in the settings file, written as `"#RRGGBB"`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An opaque sRGB color. In the settings file it is text such as `"#2F5BD3"`;
/// the hex digits may be upper or lower case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const WHITE: Color = Color::rgb(0xFF, 0xFF, 0xFF);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Red, green and blue from 0 to 1, as the GPU takes them.
    pub fn to_f32(self) -> [f32; 3] {
        [self.r, self.g, self.b].map(|v| f32::from(v) / 255.0)
    }
}

/// Text that is not a color written as `"#RRGGBB"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a color is written as \"#RRGGBB\", such as \"#2F5BD3\"")
    }
}

impl std::error::Error for ParseColorError {}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let hex = text.strip_prefix('#').ok_or(ParseColorError)?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseColorError);
        }
        let channel =
            |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).map_err(|_| ParseColorError);
        Ok(Self::rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// Always `#RRGGBB` in upper case, as the settings file writes it.
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hex_in_either_case_and_writes_upper_case() {
        let color: Color = "#2f5bD3".parse().unwrap();
        assert_eq!(color, Color::rgb(0x2F, 0x5B, 0xD3));
        assert_eq!(color.to_string(), "#2F5BD3");
    }

    #[test]
    fn anything_else_is_not_a_color() {
        for text in [
            "", "#", "2F5BD3", "#2F5BD", "#2F5BD3F", "#FFF", "#GGGGGG", "red", " #2F5BD3",
            "#+F5BD3",
        ] {
            assert_eq!(text.parse::<Color>(), Err(ParseColorError), "{text:?}");
        }
    }

    #[test]
    fn components_go_from_0_to_1() {
        assert_eq!(Color::WHITE.to_f32(), [1.0, 1.0, 1.0]);
        assert_eq!(Color::rgb(0, 51, 255).to_f32(), [0.0, 0.2, 1.0]);
    }
}
