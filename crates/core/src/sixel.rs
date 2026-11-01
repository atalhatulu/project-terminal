use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SixelImage {
    pub id: u64,
    pub col: usize,
    pub row: usize,
    pub width: usize,
    pub height: usize,
    pub rows_spanned: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    Normal,
    ColorDef,
    RepeatCount,
}

#[derive(Debug, Clone)]
pub struct SixelParser {
    pub width: usize,
    pub height: usize,
    cursor_x: usize,
    cursor_y: usize,
    max_x: usize,
    max_y: usize,
    palette: Vec<(u8, u8, u8)>,
    current_color: usize,
    pixels: Vec<u8>, // RGBA 4 bytes per pixel
    state: ParseState,
    param_buf: String,
}

impl Default for SixelParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SixelParser {
    pub fn new() -> Self {
        // Standart VT340 varsayılan 16 renk Sixel paleti
        let mut palette = vec![(0, 0, 0); 256];
        palette[0] = (0, 0, 0);
        palette[1] = (51, 102, 204);
        palette[2] = (204, 51, 51);
        palette[3] = (51, 204, 51);
        palette[4] = (204, 51, 204);
        palette[5] = (51, 204, 204);
        palette[6] = (204, 204, 51);
        palette[7] = (153, 153, 153);
        palette[8] = (68, 68, 68);
        palette[9] = (85, 136, 238);
        palette[10] = (238, 85, 85);
        palette[11] = (85, 238, 85);
        palette[12] = (238, 85, 238);
        palette[13] = (85, 238, 238);
        palette[14] = (238, 238, 85);
        palette[15] = (255, 255, 255);

        Self {
            width: 0,
            height: 0,
            cursor_x: 0,
            cursor_y: 0,
            max_x: 0,
            max_y: 0,
            palette,
            current_color: 0,
            pixels: Vec::new(),
            state: ParseState::Normal,
            param_buf: String::new(),
        }
    }

    fn ensure_pixel_capacity(&mut self, target_w: usize, target_h: usize) {
        if target_w <= self.width && target_h <= self.height {
            return;
        }

        let new_w = target_w.max(self.width).max(64);
        let new_h = target_h.max(self.height).max(64);

        let mut new_pixels = vec![0u8; new_w * new_h * 4];

        if self.width > 0 && self.height > 0 {
            for y in 0..self.height {
                let src_start = (y * self.width) * 4;
                let src_end = src_start + self.width * 4;
                let dst_start = (y * new_w) * 4;
                let dst_end = dst_start + self.width * 4;
                if src_end <= self.pixels.len() && dst_end <= new_pixels.len() {
                    new_pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
                }
            }
        }

        self.pixels = new_pixels;
        self.width = new_w;
        self.height = new_h;
    }

    fn draw_sixel_byte(&mut self, byte: u8) {
        if !(0x3F..=0x7E).contains(&byte) {
            return;
        }
        let bits = byte - 0x3F;
        let x = self.cursor_x;
        let y_base = self.cursor_y;

        self.ensure_pixel_capacity(x + 1, y_base + 6);

        let color = self.palette.get(self.current_color).copied().unwrap_or((255, 255, 255));
        self.max_x = self.max_x.max(x + 1);

        for b in 0..6 {
            if (bits & (1 << b)) != 0 {
                let y = y_base + b;
                self.max_y = self.max_y.max(y + 1);
                let idx = (y * self.width + x) * 4;
                if idx + 3 < self.pixels.len() {
                    self.pixels[idx] = color.0;
                    self.pixels[idx + 1] = color.1;
                    self.pixels[idx + 2] = color.2;
                    self.pixels[idx + 3] = 255;
                }
            }
        }
        self.cursor_x += 1;
    }

    pub fn put_byte(&mut self, byte: u8) {
        match self.state {
            ParseState::Normal => match byte {
                b'#' => {
                    self.state = ParseState::ColorDef;
                    self.param_buf.clear();
                }
                b'!' => {
                    self.state = ParseState::RepeatCount;
                    self.param_buf.clear();
                }
                b'$' => {
                    self.cursor_x = 0;
                }
                b'-' => {
                    self.cursor_x = 0;
                    self.cursor_y += 6;
                }
                0x3F..=0x7E => {
                    self.draw_sixel_byte(byte);
                }
                _ => {}
            },
            ParseState::RepeatCount => match byte {
                b'0'..=b'9' => {
                    self.param_buf.push(byte as char);
                }
                0x3F..=0x7E => {
                    let count = self.param_buf.parse::<usize>().unwrap_or(1);
                    for _ in 0..count {
                        self.draw_sixel_byte(byte);
                    }
                    self.state = ParseState::Normal;
                    self.param_buf.clear();
                }
                _ => {
                    self.state = ParseState::Normal;
                    self.param_buf.clear();
                }
            },
            ParseState::ColorDef => match byte {
                b'0'..=b'9' | b';' => {
                    self.param_buf.push(byte as char);
                }
                _ => {
                    self.parse_color_command();
                    self.state = ParseState::Normal;
                    // Eğer okunan bayt bir sixel piksel verisiyse onu da işle
                    if (0x3F..=0x7E).contains(&byte) {
                        self.draw_sixel_byte(byte);
                    } else if byte == b'$' {
                        self.cursor_x = 0;
                    } else if byte == b'-' {
                        self.cursor_x = 0;
                        self.cursor_y += 6;
                    }
                }
            },
        }
    }

    fn parse_color_command(&mut self) {
        let parts: Vec<&str> = self.param_buf.split(';').collect();
        if parts.is_empty() {
            return;
        }

        if let Ok(reg) = parts[0].parse::<usize>() {
            if parts.len() == 1 {
                // Renk seçimi (#reg)
                self.current_color = reg;
            } else if parts.len() >= 5 && parts[1] == "2" {
                // RGB renk tanımlama (#reg;2;r;g;b)
                let r = parts[2].parse::<u32>().unwrap_or(0);
                let g = parts[3].parse::<u32>().unwrap_or(0);
                let b = parts[4].parse::<u32>().unwrap_or(0);

                // Yüzdelik (0-100) veya 0-255 ölçekleme
                let r_val = if r <= 100 { (r * 255 / 100) as u8 } else { r.min(255) as u8 };
                let g_val = if g <= 100 { (g * 255 / 100) as u8 } else { g.min(255) as u8 };
                let b_val = if b <= 100 { (b * 255 / 100) as u8 } else { b.min(255) as u8 };

                if reg >= self.palette.len() {
                    self.palette.resize(reg + 1, (0, 0, 0));
                }
                self.palette[reg] = (r_val, g_val, b_val);
                self.current_color = reg;
            }
        }
        self.param_buf.clear();
    }

    pub fn finish(mut self, col: usize, row: usize, cell_height_px: usize) -> Option<SixelImage> {
        if self.state == ParseState::ColorDef {
            self.parse_color_command();
        }

        if self.max_x == 0 || self.max_y == 0 || self.pixels.is_empty() {
            return None;
        }

        let actual_w = self.max_x;
        let actual_h = self.max_y;

        // Kırpılmış nihai RGBA tamponu oluştur
        let mut cropped = Vec::with_capacity(actual_w * actual_h * 4);
        for y in 0..actual_h {
            let start = (y * self.width) * 4;
            let end = start + actual_w * 4;
            if end <= self.pixels.len() {
                cropped.extend_from_slice(&self.pixels[start..end]);
            } else {
                cropped.resize(cropped.len() + actual_w * 4, 0);
            }
        }

        static NEXT_SIXEL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_SIXEL_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let eff_cell_h = cell_height_px.max(1);
        let rows_spanned = actual_h.div_ceil(eff_cell_h);
        Some(SixelImage {
            id,
            col,
            row,
            width: actual_w,
            height: actual_h,
            rows_spanned,
            rgba: cropped,
        })
    }
}
