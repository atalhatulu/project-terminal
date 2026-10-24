use crate::grid::Grid;
use vte::{Params, Parser, Perform};

pub const DEFAULT_PALETTE: [(u8, u8, u8); 16] = [
    (21, 22, 30),      // 0: Black (#15161e)
    (247, 118, 142),   // 1: Red (#f7768e)
    (158, 206, 106),   // 2: Green (#9ece6a)
    (224, 175, 104),   // 3: Yellow (#e0af68)
    (122, 162, 247),   // 4: Blue (#7aa2f7)
    (187, 154, 247),   // 5: Magenta (#bb9af7)
    (125, 207, 255),   // 6: Cyan (#7dcfff)
    (169, 177, 214),   // 7: White (#a9b1d6)
    (65, 72, 104),     // 8: Bright Black (#414868)
    (247, 118, 142),   // 9: Bright Red
    (115, 218, 139),   // 10: Bright Green (#73daca)
    (224, 175, 104),   // 11: Bright Yellow
    (122, 162, 247),   // 12: Bright Blue
    (187, 154, 247),   // 13: Bright Magenta
    (125, 207, 255),   // 14: Bright Cyan
    (192, 202, 245),   // 15: Bright White (#c0caf5)
];

pub struct CoreParser {
    parser: Parser,
    pub primary_grid: Grid,
    pub alt_grid: Grid,
    pub use_alt: bool,
    pub command_start_time: Option<std::time::Instant>,
    pub responses: Vec<Vec<u8>>,
    pub app_cursor_keys: bool,
    pub bracketed_paste: bool,
    pub mouse_tracking: bool,
    pub mouse_sgr: bool,
    pub bell: bool,
    pub synchronized_output: bool,
    pub sync_output_start: Option<std::time::Instant>,
    pub palette: [(u8, u8, u8); 16],
    pub sixel_parser: Option<crate::sixel::SixelParser>,
    pub clipboard_store: Option<String>,
    pub keyboard_flags: u32,
    pub keyboard_flags_stack: Vec<u32>,
}

impl CoreParser {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            parser: Parser::new(),
            primary_grid: Grid::new(cols, rows),
            alt_grid: Grid::new(cols, rows),
            use_alt: false,
            command_start_time: None,
            responses: Vec::new(),
            app_cursor_keys: false,
            bracketed_paste: false,
            mouse_tracking: false,
            mouse_sgr: false,
            bell: false,
            synchronized_output: false,
            sync_output_start: None,
            palette: DEFAULT_PALETTE,
            sixel_parser: None,
            clipboard_store: None,
            keyboard_flags: 0,
            keyboard_flags_stack: Vec::new(),
        }
    }

    pub fn grid(&self) -> &Grid {
        if self.use_alt {
            &self.alt_grid
        } else {
            &self.primary_grid
        }
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.use_alt {
            &mut self.alt_grid
        } else {
            &mut self.primary_grid
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.primary_grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
    }

    pub fn set_default_colors(&mut self, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        self.primary_grid.set_default_colors(fg, bg);
        self.alt_grid.set_default_colors(fg, bg);
    }

    pub fn set_palette(&mut self, palette: [(u8, u8, u8); 16]) {
        self.palette = palette;
    }

    pub fn drain_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.responses)
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        let mut handler = CoreHandler {
            primary_grid: &mut self.primary_grid,
            alt_grid: &mut self.alt_grid,
            use_alt: &mut self.use_alt,
            command_start_time: &mut self.command_start_time,
            responses: &mut self.responses,
            app_cursor_keys: &mut self.app_cursor_keys,
            bracketed_paste: &mut self.bracketed_paste,
            mouse_tracking: &mut self.mouse_tracking,
            mouse_sgr: &mut self.mouse_sgr,
            bell: &mut self.bell,
            synchronized_output: &mut self.synchronized_output,
            sync_output_start: &mut self.sync_output_start,
            palette: &self.palette,
            sixel_parser: &mut self.sixel_parser,
            clipboard_store: &mut self.clipboard_store,
            keyboard_flags: &mut self.keyboard_flags,
            keyboard_flags_stack: &mut self.keyboard_flags_stack,
        };
        self.parser.advance(&mut handler, bytes);
    }
}

struct CoreHandler<'a> {
    primary_grid: &'a mut Grid,
    alt_grid: &'a mut Grid,
    use_alt: &'a mut bool,
    command_start_time: &'a mut Option<std::time::Instant>,
    responses: &'a mut Vec<Vec<u8>>,
    app_cursor_keys: &'a mut bool,
    bracketed_paste: &'a mut bool,
    mouse_tracking: &'a mut bool,
    mouse_sgr: &'a mut bool,
    bell: &'a mut bool,
    synchronized_output: &'a mut bool,
    sync_output_start: &'a mut Option<std::time::Instant>,
    palette: &'a [(u8, u8, u8); 16],
    sixel_parser: &'a mut Option<crate::sixel::SixelParser>,
    clipboard_store: &'a mut Option<String>,
    keyboard_flags: &'a mut u32,
    keyboard_flags_stack: &'a mut Vec<u32>,
}

impl<'a> CoreHandler<'a> {
    #[inline]
    fn grid(&mut self) -> &mut Grid {
        if *self.use_alt {
            self.alt_grid
        } else {
            self.primary_grid
        }
    }
}

#[inline]
fn palette_ansi_16_fg(palette: &[(u8, u8, u8); 16], idx: u8) -> (u8, u8, u8) {
    palette.get(idx as usize).copied().unwrap_or((192, 202, 245))
}

#[inline]
fn palette_ansi_16_bg(palette: &[(u8, u8, u8); 16], idx: u8) -> (u8, u8, u8) {
    if idx == 0 {
        (0, 0, 0)
    } else {
        palette_ansi_16_fg(palette, idx)
    }
}

#[inline]
fn palette_ansi_256(palette: &[(u8, u8, u8); 16], idx: u8) -> (u8, u8, u8) {
    match idx {
        0..=15 => palette_ansi_16_fg(palette, idx),
        16..=231 => {
            let offset = idx - 16;
            let r = (offset / 36) % 6;
            let g = (offset / 6) % 6;
            let b = offset % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}

#[inline]
fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for &b in input {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

impl<'a> Perform for CoreHandler<'a> {
    fn print(&mut self, c: char) {
        self.grid().put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        let grid = self.grid();
        match byte {
            b'\n' => grid.line_feed(),
            b'\r' => grid.carriage_return(),
            0x08 => grid.backspace(),
            b'\t' => {
                let tab_size = 8;
                let next_tab = ((grid.cursor_col / tab_size) + 1) * tab_size;
                let spaces = next_tab.saturating_sub(grid.cursor_col);
                for _ in 0..spaces {
                    grid.put_char(' ');
                }
            }
            0x07 => {
                *self.bell = true;
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        if action == 'q' {
            *self.sixel_parser = Some(crate::sixel::SixelParser::new());
        }
    }

    fn put(&mut self, byte: u8) {
        if let Some(p) = self.sixel_parser.as_mut() {
            p.put_byte(byte);
        }
    }

    fn unhook(&mut self) {
        if let Some(p) = self.sixel_parser.take() {
            let col = self.grid().cursor_col;
            let row = self.grid().cursor_row;
            if let Some(img) = p.finish(col, row) {
                let rows_spanned = (img.height + 15) / 16;
                let grid = self.grid();
                grid.cursor_row = (grid.cursor_row + rows_spanned).min(grid.rows.saturating_sub(1));
                grid.images.push(img);
                if grid.images.len() > 32 {
                    let excess = grid.images.len() - 32;
                    grid.images.drain(0..excess);
                }
                grid.mark_all_dirty();
            }
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 {
            let cmd = params[0];
            if cmd == b"0" || cmd == b"2" {
                if let Ok(t) = std::str::from_utf8(params[1]) {
                    self.grid().title = Some(t.to_string());
                }
            } else if cmd == b"8" {
                // OSC 8 ; [params] ; [url] (Yerel köprü açma / kapatma)
                let url_bytes = if params.len() >= 3 {
                    params[2]
                } else if params.len() == 2 {
                    params[1]
                } else {
                    b""
                };
                if url_bytes.is_empty() {
                    self.grid().current_hyperlink_id = None;
                } else if let Ok(url_str) = std::str::from_utf8(url_bytes) {
                    if !url_str.is_empty() {
                        let hid = self.grid().alloc_hyperlink(url_str.to_string());
                        self.grid().current_hyperlink_id = Some(hid);
                    } else {
                        self.grid().current_hyperlink_id = None;
                    }
                }
            } else if cmd == b"52" {
                // OSC 52;c;<base64> (Sistem panosuna metin kopyalama)
                let b64_payload = if params.len() >= 3 {
                    params[2]
                } else {
                    params[1]
                };
                if b64_payload != b"?" {
                    if let Some(decoded_bytes) = base64_decode(b64_payload) {
                        if let Ok(text) = String::from_utf8(decoded_bytes) {
                            *self.clipboard_store = Some(text);
                        }
                    }
                }
            } else if cmd == b"10" {
                if params[1] == b"?" {
                    let (r, g, b) = self.grid().default_fg;
                    self.responses.push(
                        format!(
                            "\x1b]10;rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x1b\\",
                            r, r, g, g, b, b
                        )
                        .into_bytes(),
                    );
                }
            } else if cmd == b"11" {
                if params[1] == b"?" {
                    let (r, g, b) = self.grid().default_bg;
                    self.responses.push(
                        format!(
                            "\x1b]11;rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x1b\\",
                            r, r, g, g, b, b
                        )
                        .into_bytes(),
                    );
                }
            } else if cmd == b"7" {
                if let Ok(uri) = std::str::from_utf8(params[1]) {
                    let path = uri.strip_prefix("file://").unwrap_or(uri);
                    let clean_path = path.find('/').map(|i| &path[i..]).unwrap_or(path);
                    self.grid().cwd = Some(clean_path.to_string());
                }
            } else if cmd == b"133" {
                if let Ok(s) = std::str::from_utf8(params[1]) {
                    let mut parts = s.split(';');
                    if let Some(tag) = parts.next() {
                        match tag {
                            "A" => {
                                // Komut başlangıcı (Prompt)
                                let new_id = self.grid().current_block_id.unwrap_or(0) + 1;
                                let cwd = self.grid().cwd.clone();
                                let cursor_row = self.grid().cursor_row;
                                let grid = self.grid();
                                grid.current_block_id = Some(new_id);
                                grid.in_command_output = false;
                                grid.blocks.push(crate::grid::CommandBlock {
                                    id: new_id,
                                    command: String::new(),
                                    cwd,
                                    start_row: cursor_row,
                                    end_row: None,
                                    exit_code: None,
                                    duration_ms: None,
                                });
                            }
                            "B" => {
                                // Komut girdisi başlangıcı
                                self.grid().in_command_output = false;
                            }
                            "C" => {
                                // Komut çalıştırma başlangıcı (Enter basıldı)
                                *self.command_start_time = Some(std::time::Instant::now());
                                // Aktif satırdaki komut metnini yakala
                                let cursor_row = self.grid().cursor_row;
                                let line = self.grid().get_display_line(cursor_row);
                                let cmd_text: String = line.iter().map(|c| c.c).collect();
                                let grid = self.grid();
                                grid.in_command_output = true;
                                if let Some(last) = grid.blocks.last_mut() {
                                    last.command = cmd_text.trim().to_string();
                                }
                            }
                            "D" => {
                                // Komut bitişi
                                let duration_ms = self.command_start_time.take().map(|t| t.elapsed().as_millis());
                                let exit_code = parts
                                    .next()
                                    .and_then(|c| c.parse::<i32>().ok())
                                    .unwrap_or(0);
                                let cursor_row = self.grid().cursor_row;
                                let grid = self.grid();
                                grid.in_command_output = false;
                                if let Some(last) = grid.blocks.last_mut() {
                                    last.end_row = Some(cursor_row);
                                    last.exit_code = Some(exit_code);
                                    last.duration_ms = duration_ms;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        if intermediates == b"?" && (action == 'h' || action == 'l') {
            for param in params.iter() {
                let mode = param.first().copied().unwrap_or(0);
                match mode {
                    1 => *self.app_cursor_keys = action == 'h',
                    25 => self.grid().cursor_visible = action == 'h',
                    1000 | 1002 | 1003 => *self.mouse_tracking = action == 'h',
                    1006 => *self.mouse_sgr = action == 'h',
                    1049 | 47 => {
                        let enable = action == 'h';
                        *self.use_alt = enable;
                        if enable {
                            self.alt_grid.clear_display(2);
                            self.alt_grid.cursor_col = 0;
                            self.alt_grid.cursor_row = 0;
                            self.alt_grid.cursor_visible = true;
                        }
                    }
                    2004 => *self.bracketed_paste = action == 'h',
                    2026 => {
                        let enable = action == 'h';
                        *self.synchronized_output = enable;
                        if enable {
                            *self.sync_output_start = Some(std::time::Instant::now());
                        } else {
                            *self.sync_output_start = None;
                            self.grid().mark_all_dirty();
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        // DECRQM (Request Mode - CSI ? 2026 $ p)
        if (intermediates == b"$" || intermediates == b"?$") && action == 'p' {
            for param in params.iter() {
                let mode = param.first().copied().unwrap_or(0);
                if mode == 2026 {
                    let status = if *self.synchronized_output { 1 } else { 2 };
                    self.responses.push(format!("\x1b[?2026;{}$y", status).into_bytes());
                }
            }
            return;
        }

        if intermediates == b"?" && action == 'n' {
            let mode = params
                .iter()
                .next()
                .and_then(|p| p.first())
                .copied()
                .unwrap_or(0);
            if mode == 6 {
                let grid = if *self.use_alt {
                    &mut *self.alt_grid
                } else {
                    &mut *self.primary_grid
                };
                let row = grid.cursor_row + 1;
                let col = grid.cursor_col + 1;
                self.responses
                    .push(format!("\x1b[?{};{}R", row, col).into_bytes());
            }
            return;
        }

        // İkincil Cihaz Nitelikleri (Secondary Device Attributes - DA2)
        if intermediates == b">" && action == 'c' {
            self.responses.push(b"\x1b[>0;115;0c".to_vec());
            return;
        }

        // Pencere Boyutu Sorgusu (CSI 18 t -> CSI 8; rows; cols t)
        if action == 't' {
            let mode = params
                .iter()
                .next()
                .and_then(|p| p.first())
                .copied()
                .unwrap_or(0);
            if mode == 18 {
                let grid = if *self.use_alt {
                    &mut *self.alt_grid
                } else {
                    &mut *self.primary_grid
                };
                self.responses
                    .push(format!("\x1b[8;{};{}t", grid.rows, grid.cols).into_bytes());
            }
            return;
        }

        // Kitty Keyboard Protocol (CSI > flags u, CSI < u, CSI = flags ; mode u, CSI ? u)
        if action == 'u' {
            if intermediates == b">" {
                // CSI > flags u : Push and set keyboard flags
                let flags = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(0) as u32;
                self.keyboard_flags_stack.push(*self.keyboard_flags);
                *self.keyboard_flags = flags;
                return;
            } else if intermediates == b"<" {
                // CSI < count u : Pop keyboard flags
                let count = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(1).max(1) as usize;
                for _ in 0..count {
                    if let Some(prev) = self.keyboard_flags_stack.pop() {
                        *self.keyboard_flags = prev;
                    } else {
                        *self.keyboard_flags = 0;
                    }
                }
                return;
            } else if intermediates == b"=" {
                // CSI = flags ; mode u : Set/replace keyboard flags
                let flags = params.iter().next().and_then(|p| p.first()).copied().unwrap_or(0) as u32;
                *self.keyboard_flags = flags;
                return;
            } else if intermediates == b"?" {
                // CSI ? u : Query current keyboard flags -> response: CSI ? flags u
                self.responses.push(format!("\x1b[?{}u", *self.keyboard_flags).into_bytes());
                return;
            }
        }

        // DECSCUSR: İmleç Şekli Belirleme (1-2: Blok, 3-4: Alt Çizgi, 5-6: Dikey Çubuk/Beam)
        if intermediates == b" " && action == 'q' {
            let shape = params
                .iter()
                .next()
                .and_then(|p| p.first())
                .copied()
                .unwrap_or(1);
            match shape {
                1 | 2 => self.grid().cursor_shape = crate::grid::CursorShape::Block,
                3 | 4 => self.grid().cursor_shape = crate::grid::CursorShape::Underline,
                5 | 6 => self.grid().cursor_shape = crate::grid::CursorShape::Beam,
                _ => self.grid().cursor_shape = crate::grid::CursorShape::Block,
            }
            return;
        }

        let grid = if *self.use_alt {
            &mut *self.alt_grid
        } else {
            &mut *self.primary_grid
        };
        grid.wrap_next = false;

        match action {
            '@' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.insert_blank_chars(n);
            }
            'A' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row = grid.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row =
                    (grid.cursor_row + n).min(grid.rows.saturating_sub(1));
            }
            'C' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_col =
                    (grid.cursor_col + n).min(grid.cols.saturating_sub(1));
            }
            'D' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_col = grid.cursor_col.saturating_sub(n);
            }
            'E' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row = (grid.cursor_row + n).min(grid.rows.saturating_sub(1));
                grid.cursor_col = 0;
            }
            'F' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row = grid.cursor_row.saturating_sub(n);
                grid.cursor_col = 0;
            }
            'G' | '`' => {
                let c = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_col = (c.saturating_sub(1)).min(grid.cols.saturating_sub(1));
            }
            'd' => {
                let r = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row = (r.saturating_sub(1)).min(grid.rows.saturating_sub(1));
            }
            'H' | 'f' => {
                let mut iter = params.iter();
                let r = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let c = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.cursor_row = (r.saturating_sub(1)).min(grid.rows.saturating_sub(1));
                grid.cursor_col = (c.saturating_sub(1)).min(grid.cols.saturating_sub(1));
            }
            'J' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                grid.clear_display(mode);
            }
            'K' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                grid.clear_line(mode);
            }
            'L' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.insert_lines(n);
            }
            'M' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.delete_lines(n);
            }
            'P' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.delete_chars(n);
            }
            'S' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.scroll_up_in_region(n);
            }
            'T' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.scroll_down_in_region(n);
            }
            'X' => {
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                grid.erase_chars(n);
            }
            's' => {
                grid.save_cursor();
            }
            'u' => {
                grid.restore_cursor();
            }
            'r' => {
                let mut iter = params.iter();
                let top = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let bottom = iter
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(grid.rows as u16)
                    .max(1) as usize;

                let top_idx = (top.saturating_sub(1)).min(grid.rows.saturating_sub(1));
                let bottom_idx = (bottom.saturating_sub(1)).min(grid.rows.saturating_sub(1));
                if top_idx < bottom_idx {
                    grid.scroll_region_top = top_idx;
                    grid.scroll_region_bottom = bottom_idx;
                }
                grid.cursor_row = 0;
                grid.cursor_col = 0;
            }
            'm' => {
                let mut values = Vec::new();
                for param in params.iter() {
                    for val in param {
                        values.push(*val);
                    }
                }
                if values.is_empty() {
                    values.push(0);
                }

                let palette = self.palette;
                let mut i = 0;
                while i < values.len() {
                    let val = values[i];
                    match val {
                        0 => {
                            grid.current_fg = grid.default_fg;
                            grid.current_bg = grid.default_bg;
                            grid.current_bold = false;
                            grid.current_italic = false;
                            grid.current_underline = false;
                            grid.current_dim = false;
                            grid.current_inverse = false;
                        }
                        1 => grid.current_bold = true,
                        2 => grid.current_dim = true,
                        3 => grid.current_italic = true,
                        4 => grid.current_underline = true,
                        7 => grid.current_inverse = true,
                        22 => {
                            grid.current_bold = false;
                            grid.current_dim = false;
                        }
                        23 => grid.current_italic = false,
                        24 => grid.current_underline = false,
                        27 => grid.current_inverse = false,
                        30..=37 => grid.current_fg = palette_ansi_16_fg(palette, val as u8 - 30),
                        38 => {
                            if i + 4 < values.len() && values[i + 1] == 2 {
                                grid.current_fg = (
                                    values[i + 2] as u8,
                                    values[i + 3] as u8,
                                    values[i + 4] as u8,
                                );
                                i += 4;
                            } else if i + 2 < values.len() && values[i + 1] == 5 {
                                grid.current_fg = palette_ansi_256(palette, values[i + 2] as u8);
                                i += 2;
                            }
                        }
                        39 => grid.current_fg = grid.default_fg,
                        40..=47 => grid.current_bg = palette_ansi_16_bg(palette, val as u8 - 40),
                        48 => {
                            if i + 4 < values.len() && values[i + 1] == 2 {
                                grid.current_bg = (
                                    values[i + 2] as u8,
                                    values[i + 3] as u8,
                                    values[i + 4] as u8,
                                );
                                i += 4;
                            } else if i + 2 < values.len() && values[i + 1] == 5 {
                                grid.current_bg = palette_ansi_256(palette, values[i + 2] as u8);
                                i += 2;
                            }
                        }
                        49 => grid.current_bg = grid.default_bg,
                        90..=97 => grid.current_fg = palette_ansi_16_fg(palette, val as u8 - 90 + 8),
                        100..=107 => grid.current_bg = palette_ansi_16_bg(palette, val as u8 - 100 + 8),
                        _ => {}
                    }
                    i += 1;
                }
            }
            'n' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                if mode == 6 {
                    // CPR (Cursor Position Report - 1 based)
                    let row = grid.cursor_row + 1;
                    let col = grid.cursor_col + 1;
                    self.responses
                        .push(format!("\x1b[{};{}R", row, col).into_bytes());
                } else if mode == 5 {
                    // Terminal status OK
                    self.responses.push(b"\x1b[0n".to_vec());
                }
            }
            'c' => {
                // Primary Device Attributes: VT220 with Sixel Graphics (code 4)
                self.responses.push(b"\x1b[?62;4;22c".to_vec());
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        let grid = self.grid();
        match byte {
            b'7' => grid.save_cursor(),
            b'8' => grid.restore_cursor(),
            b'M' => grid.reverse_index(),
            b'E' => {
                grid.cursor_col = 0;
                grid.line_feed();
            }
            b'c' => {
                let cols = grid.cols;
                let rows = grid.rows;
                *grid = Grid::new(cols, rows);
            }
            _ => {}
        }
    }
}

fn ansi_16_fg(idx: u8) -> (u8, u8, u8) {
    DEFAULT_PALETTE
        .get(idx as usize)
        .copied()
        .unwrap_or((192, 202, 245))
}


pub fn ansi_256_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0..=15 => ansi_16_fg(idx),
        16..=231 => {
            let offset = idx - 16;
            let r = (offset / 36) % 6;
            let g = (offset / 6) % 6;
            let b = offset % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}
