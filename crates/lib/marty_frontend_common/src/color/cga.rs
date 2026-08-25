#[repr(u8)]
#[derive(Debug, Copy, Clone, Default)]
pub enum CGAColor {
    #[default]
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    White,
    BlackBright,
    BlueBright,
    GreenBright,
    CyanBright,
    RedBright,
    MagentaBright,
    Yellow,
    WhiteBright,
}

impl CGAColor {
    pub fn to_rgba(&self) -> &'static [u8; 4] {
        match self {
            CGAColor::Black => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8],
            CGAColor::Blue => &[0x00u8, 0x00u8, 0xAAu8, 0xFFu8],
            CGAColor::Green => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8],
            CGAColor::Cyan => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8],
            CGAColor::Red => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8],
            CGAColor::Magenta => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8],
            CGAColor::Brown => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8],
            CGAColor::White => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8],
            CGAColor::BlackBright => &[0x55u8, 0x55u8, 0x55u8, 0xFFu8],
            CGAColor::BlueBright => &[0x55u8, 0x55u8, 0xFFu8, 0xFFu8],
            CGAColor::GreenBright => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8],
            CGAColor::CyanBright => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8],
            CGAColor::RedBright => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8],
            CGAColor::MagentaBright => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8],
            CGAColor::Yellow => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8],
            CGAColor::WhiteBright => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8],
        }
    }

    pub fn decode_attr(attr: u8) -> (CGAColor, CGAColor) {
        let fg = match attr & 0x0F {
            0x00 => CGAColor::Black,
            0x01 => CGAColor::Blue,
            0x02 => CGAColor::Green,
            0x03 => CGAColor::Cyan,
            0x04 => CGAColor::Red,
            0x05 => CGAColor::Magenta,
            0x06 => CGAColor::Brown,
            0x07 => CGAColor::White,
            0x08 => CGAColor::BlackBright,
            0x09 => CGAColor::BlueBright,
            0x0A => CGAColor::GreenBright,
            0x0B => CGAColor::CyanBright,
            0x0C => CGAColor::RedBright,
            0x0D => CGAColor::MagentaBright,
            0x0E => CGAColor::Yellow,
            0x0F => CGAColor::WhiteBright,
            _ => CGAColor::Black,
        };
        let bg = match (attr & 0xF0) >> 4 {
            0x00 => CGAColor::Black,
            0x01 => CGAColor::Blue,
            0x02 => CGAColor::Green,
            0x03 => CGAColor::Cyan,
            0x04 => CGAColor::Red,
            0x05 => CGAColor::Magenta,
            0x06 => CGAColor::Brown,
            0x07 => CGAColor::White,
            0x08 => CGAColor::BlackBright,
            0x09 => CGAColor::BlueBright,
            0x0A => CGAColor::GreenBright,
            0x0B => CGAColor::CyanBright,
            0x0C => CGAColor::RedBright,
            0x0D => CGAColor::MagentaBright,
            0x0E => CGAColor::Yellow,
            0x0F => CGAColor::WhiteBright,
            _ => CGAColor::Black,
        };
        (fg, bg)
    }
}
