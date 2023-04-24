use std::cmp::{Eq, Ord, PartialOrd};
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::iter;
use std::ops::RangeInclusive;

use image::Rgba;

macro_rules! make_ranged_int {
    ($name:ident, $type:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Hash, Eq, Ord)]
        pub struct $name<const MIN: $type, const MAX: $type>($type);

        impl<const MIN: $type, const MAX: $type> $name<MIN, MAX> {
            const VAL_ASSERTION: () = assert!(MIN <= MAX);

            pub const fn new(value: $type) -> Option<Self> {
                if value >= MIN && value <= MAX {
                    Some(Self(value))
                } else {
                    None
                }
            }

            pub const fn get(self) -> $type {
                self.0
            }

            pub const fn range() -> RangeInclusive<$type> {
                MIN..=MAX
            }
        }

        impl<const MIN: $type, const MAX: $type> Debug for $name<MIN, MAX> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl<const MIN: $type, const MAX: $type> PartialEq for $name<MIN, MAX> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl<const MIN: $type, const MAX: $type> PartialOrd for $name<MIN, MAX> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.0.partial_cmp(&other.0)
            }
        }

        impl<const MIN: $type, const MAX: $type> serde::Serialize for $name<MIN, MAX> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                if self.0 < MIN || self.0 > MAX {
                    Err(serde::ser::Error::custom(format!(
                        "Value {} is not in range {}..={}",
                        self.0, MIN, MAX
                    )))
                } else {
                    self.0.serialize(serializer)
                }
            }
        }

        impl<'de, const MIN: $type, const MAX: $type> serde::Deserialize<'de> for $name<MIN, MAX> {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <$type>::deserialize(deserializer)?;
                if value < MIN || value > MAX {
                    Err(serde::de::Error::custom(format!(
                        "Value {} is not in range {}..={}",
                        value, MIN, MAX
                    )))
                } else {
                    Ok(Self(value))
                }
            }
        }

        impl<const MIN: $type, const MAX: $type> Into<$type> for $name<MIN, MAX> {
            fn into(self) -> $type {
                self.0
            }
        }
    };
}

make_ranged_int!(RangedU8, u8);
make_ranged_int!(RangedU16, u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba32(rgba: u32) -> Self {
        Self {
            r: ((rgba >> 24) & 0xFF) as u8,
            g: ((rgba >> 16) & 0xFF) as u8,
            b: ((rgba >> 8) & 0xFF) as u8,
            a: (rgba & 0xFF) as u8,
        }
    }

    /// Parses a color from a string in the format `#rrggbb` or `#rrggbbaa`.
    pub fn parse(s: &str) -> Option<Self> {
        if s.len() != 7 && s.len() != 9 {
            return None;
        }

        if !s.starts_with('#') {
            return None;
        }

        let mut chars = s[1..].chars();

        let r = chars.next()?.to_digit(16)?;
        let r = ((r << 4) + chars.next()?.to_digit(16)?) as u8;

        let g = chars.next()?.to_digit(16)?;
        let g = ((g << 4) + chars.next()?.to_digit(16)?) as u8;

        let b = chars.next()?.to_digit(16)?;
        let b = ((b << 4) + chars.next()?.to_digit(16)?) as u8;

        let a = if s.len() == 9 {
            let a = chars.next()?.to_digit(16)?;
            ((a << 4) + chars.next()?.to_digit(16)?) as u8
        } else {
            255
        };

        println!("{} {} {} {}", r, g, b, a);

        Some(Self { r, g, b, a })
    }

    pub const fn into_rgba(&self) -> Rgba<u8> {
        Rgba([self.r, self.g, self.b, self.a])
    }
}

impl serde::Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = String::with_capacity(9);

        s.push('#');
        let Color { r, g, b, a } = *self;
        s.push(char::from_digit((r >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((r & 0xf) as u32, 16).unwrap());
        s.push(char::from_digit((g >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((g & 0xf) as u32, 16).unwrap());
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());

        if a != 255 {
            s.push(char::from_digit((a >> 4) as u32, 16).unwrap());
            s.push(char::from_digit((a & 0xf) as u32, 16).unwrap());
        }

        s.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom("Invalid color"))
    }
}
