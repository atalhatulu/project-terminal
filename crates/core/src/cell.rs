use serde::{Deserialize, Serialize};
use std::num::NonZeroU16;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnderlineStyle {
    None = 0,
    Single = 1,
    Double = 2,
    Curly = 3,
    Dotted = 4,
    Dashed = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub c: char,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub inverse: bool,
    pub is_output: bool,
    pub strikethrough: bool,
    pub underline_style: UnderlineStyle,
    pub block_id: Option<NonZeroU16>,
    pub hyperlink_id: Option<NonZeroU16>,
    pub underline_color: Option<NonZeroU16>,
}

#[inline]
fn rgb_to_565(r: u8, g: u8, b: u8) -> NonZeroU16 {
    let r5 = (r as u16 >> 3) & 0x1F;
    let g6 = (g as u16 >> 2) & 0x3F;
    let b5 = (b as u16 >> 3) & 0x1F;
    let val = (r5 << 11) | (g6 << 5) | b5;
    // Ensure non-zero by mapping 0 to 1
    NonZeroU16::new(if val == 0 { 1 } else { val }).unwrap()
}

#[inline]
fn rgb_from_565(val: NonZeroU16) -> (u8, u8, u8) {
    let v = val.get();
    let r5 = (v >> 11) & 0x1F;
    let g6 = (v >> 5) & 0x3F;
    let b5 = v & 0x1F;
    let r = ((r5 * 527 + 23) >> 6) as u8;
    let g = ((g6 * 259 + 33) >> 6) as u8;
    let b = ((b5 * 527 + 23) >> 6) as u8;
    (r, g, b)
}

impl Cell {
    #[inline]
    pub fn get_block_id(&self) -> Option<u32> {
        self.block_id.map(|n| n.get() as u32)
    }

    #[inline]
    pub fn get_hyperlink_id(&self) -> Option<u32> {
        self.hyperlink_id.map(|n| n.get() as u32)
    }

    #[inline]
    pub fn underline_color_rgb(&self) -> Option<(u8, u8, u8)> {
        self.underline_color.map(rgb_from_565)
    }

    #[inline]
    pub fn set_underline_color_rgb(&mut self, color: Option<(u8, u8, u8)>) {
        self.underline_color = color.map(|(r, g, b)| rgb_to_565(r, g, b));
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: (226, 228, 234), // Crisp off-white (#e2e4ea)
            bg: (0, 0, 0),       // Deep OLED Black (#000000)
            bold: false,
            italic: false,
            underline: false,
            dim: false,
            inverse: false,
            is_output: false,
            strikethrough: false,
            underline_style: UnderlineStyle::None,
            block_id: None,
            hyperlink_id: None,
            underline_color: None,
        }
    }
}
