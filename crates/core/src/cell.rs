use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

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
    pub block_id: Option<NonZeroU32>,
    pub hyperlink_id: Option<NonZeroU32>,
}

impl Cell {
    #[inline]
    pub fn get_block_id(&self) -> Option<u32> {
        self.block_id.map(|n| n.get())
    }

    #[inline]
    pub fn get_hyperlink_id(&self) -> Option<u32> {
        self.hyperlink_id.map(|n| n.get())
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
            block_id: None,
            hyperlink_id: None,
        }
    }
}
