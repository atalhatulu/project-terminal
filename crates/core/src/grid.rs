use crate::cell::Cell;
use crate::selection::Selection;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

/// Sürekli ve yerinde bellek geri dönüşümlü dairesel kaydırma arabelleği (Circular Scrollback Ring Buffer)
/// Satırlar dolduğunda bellek boşaltılıp yeniden tahsis edilmez; en eski satırın ayrılmış belleği
/// sıfır ek bellek ayırma (0 allocation) ile doğrudan üzerine yazılır.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollbackRing {
    entries: Vec<Vec<Cell>>,
    head: usize,
    len: usize,
    capacity: usize,
}

impl ScrollbackRing {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity.min(1024)),
            head: 0,
            len: 0,
            capacity: capacity.max(1),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Yeni satırı arabelleğe ekler. Kapasite dolduğunda en eski satırın belleği
    /// sıfır bellek ayırma (0 allocation) ile yerinde doğrudan güncellenir.
    pub fn push(&mut self, row: &[Cell]) {
        if self.len < self.capacity {
            if self.entries.len() < self.capacity {
                self.entries.push(row.to_vec());
            } else {
                let slot = &mut self.entries[self.len];
                slot.clear();
                slot.extend_from_slice(row);
            }
            self.len += 1;
        } else {
            // Halka doldu: En eski slotu (head) yerinde güncelle ve başı ilerlet
            let slot = &mut self.entries[self.head];
            slot.clear();
            slot.extend_from_slice(row);
            self.head = (self.head + 1) % self.capacity;
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&Vec<Cell>> {
        if index < self.len {
            let phys = (self.head + index) % self.entries.len();
            Some(&self.entries[phys])
        } else {
            None
        }
    }

    #[inline]
    pub fn iter(&self) -> ScrollbackIter<'_> {
        ScrollbackIter {
            ring: self,
            current: 0,
        }
    }
}

impl std::ops::Index<usize> for ScrollbackRing {
    type Output = Vec<Cell>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!("Scrollback indeksi sınırların dışında: {} >= {}", index, self.len);
        }
        let phys = (self.head + index) % self.entries.len();
        &self.entries[phys]
    }
}

pub struct ScrollbackIter<'a> {
    ring: &'a ScrollbackRing,
    current: usize,
}

impl<'a> Iterator for ScrollbackIter<'a> {
    type Item = &'a Vec<Cell>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.ring.len {
            let item = &self.ring[self.current];
            self.current += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.ring.len - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> IntoIterator for &'a ScrollbackRing {
    type Item = &'a Vec<Cell>;
    type IntoIter = ScrollbackIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandBlock {
    pub id: u32,
    pub command: String,
    pub cwd: Option<String>,
    pub start_row: usize,
    pub end_row: Option<usize>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineDamage {
    pub row: usize,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub lines: Vec<Vec<Cell>>,
    pub blocks: Vec<CommandBlock>,
    pub title: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    Block,
    Beam,
    Underline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedCursor {
    pub col: usize,
    pub row: usize,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub underline_style: crate::cell::UnderlineStyle,
    pub underline_color: Option<(u8, u8, u8)>,
    pub strikethrough: bool,
    pub dim: bool,
    pub inverse: bool,
}

pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    pub scrollback: ScrollbackRing,
    pub scroll_offset: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    pub default_fg: (u8, u8, u8),
    pub default_bg: (u8, u8, u8),
    pub current_fg: (u8, u8, u8),
    pub current_bg: (u8, u8, u8),
    pub current_bold: bool,
    pub current_italic: bool,
    pub current_underline: bool,
    pub current_underline_style: crate::cell::UnderlineStyle,
    pub current_underline_color: Option<(u8, u8, u8)>,
    pub current_strikethrough: bool,
    pub current_dim: bool,
    pub current_inverse: bool,
    pub saved_cursor: Option<SavedCursor>,
    pub current_block_id: Option<u32>,
    pub in_command_output: bool,
    pub blocks: Vec<CommandBlock>,
    pub selection: Option<Selection>,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub dirty_lines: Vec<bool>,
    pub scroll_region_top: usize,
    pub scroll_region_bottom: usize,
    pub wrap_next: bool,
    pub search_query: Option<String>,
    pub search_matches: Vec<SearchMatch>,
    pub active_match_idx: usize,
    pub images: Vec<crate::sixel::SixelImage>,
    pub version: u64,
    pub current_hyperlink_id: Option<u32>,
    pub hyperlinks: std::collections::HashMap<u32, String>,
    pub hyperlink_fifo: std::collections::VecDeque<u32>,
    pub next_hyperlink_id: u32,
    pub empty_row: Vec<Cell>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let size = cols * rows;
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); size],
            scrollback: ScrollbackRing::with_capacity(10000),
            scroll_offset: 0,
            cursor_col: 0,
            cursor_row: 0,
            cursor_visible: true,
            cursor_shape: CursorShape::Block,
            default_fg: (226, 228, 234), // Crisp off-white (#e2e4ea)
            default_bg: (0, 0, 0),       // Deep OLED Black (#000000)
            current_fg: (226, 228, 234),
            current_bg: (0, 0, 0),
            current_bold: false,
            current_italic: false,
            current_underline: false,
            current_underline_style: crate::cell::UnderlineStyle::None,
            current_underline_color: None,
            current_strikethrough: false,
            current_dim: false,
            current_inverse: false,
            saved_cursor: None,
            current_block_id: None,
            in_command_output: false,
            blocks: Vec::new(),
            selection: None,
            title: None,
            cwd: None,
            dirty_lines: vec![true; rows],
            images: Vec::new(),
            scroll_region_top: 0,
            scroll_region_bottom: rows.saturating_sub(1),
            wrap_next: false,
            search_query: None,
            search_matches: Vec::new(),
            active_match_idx: 0,
            version: 1,
            current_hyperlink_id: None,
            hyperlinks: std::collections::HashMap::new(),
            hyperlink_fifo: std::collections::VecDeque::new(),
            next_hyperlink_id: 1,
            empty_row: vec![Cell::default(); cols],
        }
    }

    pub fn alloc_hyperlink(&mut self, url: String) -> u32 {
        let id = self.next_hyperlink_id;
        self.next_hyperlink_id = self.next_hyperlink_id.wrapping_add(1).max(1);
        self.hyperlinks.insert(id, url);
        self.hyperlink_fifo.push_back(id);
        if self.hyperlinks.len() > 1024 {
            for _ in 0..128 {
                if let Some(old_id) = self.hyperlink_fifo.pop_front() {
                    self.hyperlinks.remove(&old_id);
                }
            }
        }
        id
    }

    /// Varsayılan ön plan ve arka plan renklerini günceller.
    ///
    /// Not: Sadece boşluk (' ') karakterine sahip arka plan hücreleri yeni varsayılan renklerle güncellenir.
    /// Kullanıcı veya kabuk tarafından açıkça renklendirilmiş (SGR dizileriyle belirlenmiş) metin ve hücreler
    /// stillerini korumak amacıyla bilerek değiştirilmez.
    pub fn set_default_colors(&mut self, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        self.default_fg = fg;
        self.default_bg = bg;
        self.current_fg = fg;
        self.current_bg = bg;
        for cell in &mut self.cells {
            if cell.c == ' ' {
                cell.fg = fg;
                cell.bg = bg;
            }
        }
    }

    pub fn mark_line_dirty(&mut self, row: usize) {
        if row < self.dirty_lines.len() {
            self.dirty_lines[row] = true;
            self.version = self.version.wrapping_add(1);
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for d in &mut self.dirty_lines {
            *d = true;
        }
        self.version = self.version.wrapping_add(1);
    }

    pub fn set_selection(&mut self, selection: Option<Selection>) {
        if self.selection != selection {
            self.selection = selection;
            self.mark_all_dirty();
        }
    }

    pub fn extract_damage(&mut self) -> Vec<LineDamage> {
        let dirty_indices: Vec<usize> = self
            .dirty_lines
            .iter_mut()
            .enumerate()
            .filter_map(|(r, is_dirty)| {
                if *is_dirty {
                    *is_dirty = false;
                    Some(r)
                } else {
                    None
                }
            })
            .collect();

        let mut damage = Vec::with_capacity(dirty_indices.len());
        for r in dirty_indices {
            let line_cells = self.get_display_line(r);
            damage.push(LineDamage {
                row: r,
                cells: line_cells.to_vec(),
            });
        }
        damage
    }

    pub fn snapshot(&self) -> GridSnapshot {
        let mut lines = Vec::with_capacity(self.rows);
        for r in 0..self.rows {
            let line_cells = self.get_display_line(r);
            lines.push(line_cells.to_vec());
        }
        GridSnapshot {
            cols: self.cols,
            rows: self.rows,
            cursor_col: self.cursor_col,
            cursor_row: self.cursor_row,
            lines,
            blocks: self.blocks.clone(),
            title: self.title.clone(),
            cwd: self.cwd.clone(),
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if self.cols == cols && self.rows == rows {
            return;
        }
        let mut new_cells = vec![Cell::default(); cols * rows];
        let min_cols = self.cols.min(cols);
        let min_rows = self.rows.min(rows);

        for r in 0..min_rows {
            for c in 0..min_cols {
                let old_idx = r * self.cols + c;
                let new_idx = r * cols + c;
                new_cells[new_idx] = self.cells[old_idx];
            }
        }

        self.cols = cols;
        self.rows = rows;
        self.cells = new_cells;
        self.empty_row = vec![Cell::default(); cols];
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
        self.selection = None;
        self.dirty_lines = vec![true; rows];
        self.scroll_region_top = 0;
        self.scroll_region_bottom = rows.saturating_sub(1);
        self.wrap_next = false;
        self.images.retain(|img| img.col < cols && img.row < rows);
    }

#[inline]
pub fn combine_chars(base: char, combining: char) -> Option<char> {
    unicode_normalization::char::compose(base, combining)
}

    pub fn put_char(&mut self, c: char) {
        if self.wrap_next {
            self.carriage_return();
            self.line_feed();
            self.wrap_next = false;
        }

        let width = c.width().unwrap_or(1);
        if width == 0 {
            // Sıfır genişlikli birleşik karakter (combining character / diacritic / ZWJ):
            // İmleci ilerletmeden önceki hücrenin üzerine ekleme / birleştirme yap
            if self.cursor_col > 0 {
                let prev_col = self.cursor_col - 1;
                let prev_idx = self.cursor_row * self.cols + prev_col;
                if prev_idx < self.cells.len() {
                    let target_idx = if self.cells[prev_idx].c == '\0' && prev_col > 0 {
                        self.cursor_row * self.cols + (prev_col - 1)
                    } else {
                        prev_idx
                    };
                    let prev_c = self.cells[target_idx].c;
                    if c == '\u{200D}' {
                        // ZWJ: Önceki hücreyi kirli işaretle, bir sonraki karakterle birleşmeyi destekle
                        self.mark_line_dirty(self.cursor_row);
                    } else if let Some(composed) = Self::combine_chars(prev_c, c) {
                        self.cells[target_idx].c = composed;
                        self.mark_line_dirty(self.cursor_row);
                    }
                }
            }
            return;
        }

        if self.cursor_col + width > self.cols {
            self.carriage_return();
            self.line_feed();
            self.wrap_next = false;
        }

        let b_id = self.current_block_id.and_then(|id| std::num::NonZeroU16::new(id as u16));
        let h_id = self.current_hyperlink_id.and_then(|id| std::num::NonZeroU16::new(id as u16));
        let is_out = self.in_command_output && self.current_block_id.is_some();

        let mut cell = Cell {
            c,
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            underline_style: self.current_underline_style,
            underline_color: None,
            strikethrough: self.current_strikethrough,
            dim: self.current_dim,
            inverse: self.current_inverse,
            is_output: is_out,
            block_id: b_id,
            hyperlink_id: h_id,
        };
        cell.set_underline_color_rgb(self.current_underline_color);

        let idx = self.cursor_row * self.cols + self.cursor_col;
        if idx < self.cells.len() {
            self.cells[idx] = cell;
        }

        if width == 2 && self.cursor_col + 1 < self.cols {
            let next_idx = self.cursor_row * self.cols + self.cursor_col + 1;
            if next_idx < self.cells.len() {
                let mut next_cell = cell;
                next_cell.c = '\0';
                self.cells[next_idx] = next_cell;
            }
        }

        self.mark_line_dirty(self.cursor_row);

        if self.cursor_col + width >= self.cols {
            self.cursor_col = self.cols.saturating_sub(1);
            self.wrap_next = true;
        } else {
            self.cursor_col += width;
            self.wrap_next = false;
        }
    }

    pub fn delete_chars(&mut self, count: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col >= self.cols || count == 0 {
            return;
        }
        let n = count.min(self.cols - col);
        let row_start = row * self.cols;
        for c in col..(self.cols - n) {
            self.cells[row_start + c] = self.cells[row_start + c + n];
        }
        for c in (self.cols - n)..self.cols {
            self.cells[row_start + c] = Cell::default();
        }
        self.mark_line_dirty(row);
    }

    pub fn insert_blank_chars(&mut self, count: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col >= self.cols || count == 0 {
            return;
        }
        let n = count.min(self.cols - col);
        let row_start = row * self.cols;
        for c in (col + n..self.cols).rev() {
            self.cells[row_start + c] = self.cells[row_start + c - n];
        }
        for c in col..(col + n) {
            self.cells[row_start + c] = Cell::default();
        }
        self.mark_line_dirty(row);
    }

    pub fn erase_chars(&mut self, count: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col >= self.cols || count == 0 {
            return;
        }
        let end_col = (col + count).min(self.cols);
        let row_start = row * self.cols;
        for c in col..end_col {
            self.cells[row_start + c] = Cell::default();
        }
        self.mark_line_dirty(row);
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(SavedCursor {
            col: self.cursor_col,
            row: self.cursor_row,
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            underline_style: self.current_underline_style,
            underline_color: self.current_underline_color,
            strikethrough: self.current_strikethrough,
            dim: self.current_dim,
            inverse: self.current_inverse,
        });
    }

    pub fn restore_cursor(&mut self) {
        if let Some(saved) = self.saved_cursor.clone() {
            self.cursor_col = saved.col.min(self.cols.saturating_sub(1));
            self.cursor_row = saved.row.min(self.rows.saturating_sub(1));
            self.current_fg = saved.fg;
            self.current_bg = saved.bg;
            self.current_bold = saved.bold;
            self.current_italic = saved.italic;
            self.current_underline = saved.underline;
            self.current_underline_style = saved.underline_style;
            self.current_underline_color = saved.underline_color;
            self.current_strikethrough = saved.strikethrough;
            self.current_dim = saved.dim;
            self.current_inverse = saved.inverse;
        }
    }

    pub fn reverse_index(&mut self) {
        if self.cursor_row == self.scroll_region_top {
            self.scroll_down_in_region(1);
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
        self.mark_line_dirty(self.cursor_row);
    }

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
        self.wrap_next = false;
    }

    pub fn line_feed(&mut self) {
        self.wrap_next = false;
        if self.cursor_row < self.scroll_region_bottom {
            self.cursor_row += 1;
        } else if self.cursor_row == self.scroll_region_bottom {
            self.scroll_up_in_region(1);
        } else if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        }
        self.mark_line_dirty(self.cursor_row);
    }

    pub fn backspace(&mut self) {
        self.wrap_next = false;
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn scroll_up_in_region(&mut self, n: usize) {
        let top = self.scroll_region_top;
        let bottom = self.scroll_region_bottom;
        if top >= bottom || n == 0 {
            return;
        }

        let cols = self.cols;
        let n = n.min(bottom - top + 1);

        // En üst satırı scrollback'e at (sadece tüm ekran kaydırılıyorsa)
        if top == 0 {
            for i in 0..n {
                let row_idx = i;
                let slice = &self.cells[row_idx * cols..(row_idx + 1) * cols];
                self.scrollback.push(slice);
            }
        }

        // Satırları yukarı kaydır
        let move_count = (bottom - top + 1).saturating_sub(n);
        if move_count > 0 {
            let src = (top + n) * cols;
            let dst = top * cols;
            let len = move_count * cols;
            self.cells.copy_within(src..src + len, dst);
        }

        // Altta kalan satırları temizle
        let default_cell = Cell::default();

        for r in (bottom + 1 - n)..=bottom {
            for c in 0..cols {
                let idx = r * cols + c;
                if idx < self.cells.len() {
                    self.cells[idx] = default_cell;
                }
            }
        }

        // Görsellerin konumlarını yukarı kaydır, ekran dışına çıkanları temizle
        self.images.retain_mut(|img| {
            if img.row < top || img.row > bottom {
                return true;
            }
            if img.row >= top + n {
                img.row -= n;
                true
            } else {
                let passed = (top + n) - img.row;
                if img.rows_spanned <= passed {
                    false
                } else {
                    img.rows_spanned -= passed;
                    img.top_offset_rows += passed;
                    img.row = top;
                    true
                }
            }
        });

        // Maksimum görsel kapasitesini sınırla (bellek patlamasını önle)
        if self.images.len() > 32 {
            let excess = self.images.len() - 32;
            self.images.drain(0..excess);
        }

        self.mark_all_dirty();
    }

    pub fn scroll_down_in_region(&mut self, n: usize) {
        let top = self.scroll_region_top;
        let bottom = self.scroll_region_bottom;
        if top >= bottom || n == 0 {
            return;
        }

        let cols = self.cols;
        let n = n.min(bottom - top + 1);
        let move_count = (bottom - top + 1).saturating_sub(n);

        if move_count > 0 {
            let src = top * cols;
            let dst = (top + n) * cols;
            let len = move_count * cols;
            self.cells.copy_within(src..src + len, dst);
        }

        let default_cell = Cell::default();

        for r in top..(top + n) {
            for c in 0..cols {
                let idx = r * cols + c;
                if idx < self.cells.len() {
                    self.cells[idx] = default_cell;
                }
            }
        }

        // Görsellerin konumlarını aşağı kaydır
        self.images.retain_mut(|img| {
            if img.row < top || img.row > bottom {
                return true;
            }
            img.row += n;
            if img.row > bottom {
                return false;
            }
            if img.row + img.rows_spanned > bottom + 1 {
                img.rows_spanned = (bottom + 1).saturating_sub(img.row);
            }
            img.rows_spanned > 0
        });

        self.mark_all_dirty();
    }

    pub fn insert_lines(&mut self, count: usize) {
        if self.cursor_row >= self.scroll_region_top && self.cursor_row <= self.scroll_region_bottom {
            let saved_top = self.scroll_region_top;
            self.scroll_region_top = self.cursor_row;
            self.scroll_down_in_region(count);
            self.scroll_region_top = saved_top;
        }
    }

    pub fn delete_lines(&mut self, count: usize) {
        if self.cursor_row >= self.scroll_region_top && self.cursor_row <= self.scroll_region_bottom {
            let saved_top = self.scroll_region_top;
            self.scroll_region_top = self.cursor_row;
            self.scroll_up_in_region(count);
            self.scroll_region_top = saved_top;
        }
    }

    pub fn clear_display(&mut self, mode: u16) {
        let r = self.cursor_row;
        let c = self.cursor_col;
        let default_cell = Cell::default();
        match mode {
            0 => {
                for col in c..self.cols {
                    let idx = r * self.cols + col;
                    if idx < self.cells.len() {
                        self.cells[idx] = default_cell;
                    }
                }
                for row in (r + 1)..self.rows {
                    for col in 0..self.cols {
                        let idx = row * self.cols + col;
                        if idx < self.cells.len() {
                            self.cells[idx] = default_cell;
                        }
                    }
                }
            }
            1 => {
                for row in 0..r {
                    for col in 0..self.cols {
                        let idx = row * self.cols + col;
                        if idx < self.cells.len() {
                            self.cells[idx] = default_cell;
                        }
                    }
                }
                for col in 0..=c {
                    let idx = r * self.cols + col;
                    if idx < self.cells.len() {
                        self.cells[idx] = default_cell;
                    }
                }
            }
            2 => {
                for cell in &mut self.cells {
                    *cell = default_cell;
                }
                self.selection = None;
                self.images.clear();
            }
            3 => {
                self.scrollback.clear();
                self.scroll_offset = 0;
                self.images.clear();
            }
            _ => {}
        }
        self.mark_all_dirty();
    }

    pub fn clear_line(&mut self, mode: u16) {
        let r = self.cursor_row;
        let c = self.cursor_col;
        let default_cell = Cell::default();

        match mode {
            0 => {
                for col in c..self.cols {
                    let idx = r * self.cols + col;
                    if idx < self.cells.len() {
                        self.cells[idx] = default_cell;
                    }
                }
            }
            1 => {
                for col in 0..=c {
                    let idx = r * self.cols + col;
                    if idx < self.cells.len() {
                        self.cells[idx] = default_cell;
                    }
                }
            }
            2 => {
                for col in 0..self.cols {
                    let idx = r * self.cols + col;
                    if idx < self.cells.len() {
                        self.cells[idx] = default_cell;
                    }
                }
            }
            _ => {}
        }
        self.mark_line_dirty(r);
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.cursor_col = 0;
        self.cursor_row = 0;
        self.selection = None;
        self.mark_all_dirty();
    }

    pub fn scroll(&mut self, delta: i32) {
        if delta > 0 {
            let max_scroll = self.scrollback.len();
            self.scroll_offset = (self.scroll_offset + delta as usize).min(max_scroll);
        } else if delta < 0 {
            let sub = (-delta) as usize;
            self.scroll_offset = self.scroll_offset.saturating_sub(sub);
        }
        self.mark_all_dirty();
    }

    pub fn get_display_line(&self, r: usize) -> &[Cell] {
        if self.scroll_offset == 0 {
            let start = r * self.cols;
            let end = start + self.cols;
            if end <= self.cells.len() {
                return &self.cells[start..end];
            }
            return &self.empty_row;
        }

        let total_scroll = self.scrollback.len();
        let virtual_top = total_scroll.saturating_sub(self.scroll_offset);
        let target_idx = virtual_top + r;

        if target_idx < total_scroll {
            &self.scrollback[target_idx]
        } else {
            let active_r = target_idx - total_scroll;
            if active_r < self.rows {
                let start = active_r * self.cols;
                let end = start + self.cols;
                if end <= self.cells.len() {
                    return &self.cells[start..end];
                }
            }
            &self.empty_row
        }
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let ((s_col, s_row), (e_col, e_row)) = sel.normalized();
        let mut result = String::new();

        for r in s_row..=e_row {
            let col_start = if r == s_row { s_col } else { 0 };
            let col_end = if r == e_row {
                e_col
            } else {
                self.cols.saturating_sub(1)
            };

            let line_cells = self.get_display_line(r);
            let mut line = String::new();
            for c in col_start..=col_end {
                if c < line_cells.len() && line_cells[c].c != '\0' {
                    line.push(line_cells[c].c);
                }
            }
            if r > s_row {
                result.push('\n');
            }
            result.push_str(line.trim_end());
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    pub fn select_word(&mut self, col: usize, row: usize) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let line_cells = self.get_display_line(row);
        if col >= line_cells.len() {
            return;
        }
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == '.';
        if !is_word_char(line_cells[col].c) {
            self.selection = Some(Selection::new(col, row, col, row));
            return;
        }

        let mut start_c = col;
        while start_c > 0 && is_word_char(line_cells[start_c - 1].c) {
            start_c -= 1;
        }
        let mut end_c = col;
        while end_c + 1 < line_cells.len() && is_word_char(line_cells[end_c + 1].c) {
            end_c += 1;
        }
        self.selection = Some(Selection::new(start_c, row, end_c, row));
        self.mark_line_dirty(row);
    }

    pub fn select_line(&mut self, row: usize) {
        if row >= self.rows {
            return;
        }
        self.selection = Some(Selection::new(0, row, self.cols.saturating_sub(1), row));
        self.mark_line_dirty(row);
    }

    /// Belirtilen hücre koordinatında (col, row) tıklanabilir bir URL varsa döndürür (Önce OSC 8, sonra metin taraması).
    pub fn get_url_at(&self, col: usize, row: usize) -> Option<String> {
        let line = self.get_display_line(row);
        if col < line.len() {
            if let Some(hid) = line[col].get_hyperlink_id() {
                if let Some(url) = self.hyperlinks.get(&hid) {
                    return Some(url.clone());
                }
            }
        }

        let text: String = line.iter().map(|c| c.c).collect();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();

        let mut idx = 0;
        while idx < len {
            let remaining: String = chars[idx..].iter().collect();
            if remaining.starts_with("http://") || remaining.starts_with("https://") {
                let start = idx;
                let mut end = start;
                while end < len {
                    let ch = chars[end];
                    if ch.is_whitespace()
                        || ch == '"'
                        || ch == '\''
                        || ch == '<'
                        || ch == '>'
                        || ch == '`'
                        || ch == '['
                        || ch == ']'
                        || ch == '\0'
                    {
                        break;
                    }
                    end += 1;
                }
                // Sondaki noktalama işaretlerini (. , ; : )) buda
                let mut trimmed_end = end;
                while trimmed_end > start {
                    let last = chars[trimmed_end - 1];
                    if last == '.' || last == ',' || last == ';' || last == ':' || last == ')' {
                        trimmed_end -= 1;
                    } else {
                        break;
                    }
                }
                if col >= start && col < trimmed_end {
                    let url: String = chars[start..trimmed_end].iter().collect();
                    return Some(url);
                }
                idx = end;
            } else {
                idx += 1;
            }
        }
        None
    }

    /// Belirtilen koordinattaki (col, row) diskte var olan bir dosya yolunu (:satır:sütun formatı dahil) algılar.
    pub fn get_file_target_at(&self, col: usize, row: usize) -> Option<FileTarget> {
        let line = self.get_display_line(row);
        let text: String = line.iter().map(|c| c.c).collect();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();

        if col >= len || chars[col].is_whitespace() || chars[col] == '\0' {
            return None;
        }

        let is_sep = |c: char| {
            c.is_whitespace()
                || c == '"'
                || c == '\''
                || c == '`'
                || c == '<'
                || c == '>'
                || c == '['
                || c == ']'
                || c == '('
                || c == ')'
                || c == '{'
                || c == '}'
                || c == '\0'
        };

        let mut start = col;
        while start > 0 && !is_sep(chars[start - 1]) {
            start -= 1;
        }
        let mut end = col;
        while end < len && !is_sep(chars[end]) {
            end += 1;
        }

        while end > start && (chars[end - 1] == '.' || chars[end - 1] == ',' || chars[end - 1] == ';') {
            end -= 1;
        }

        if start >= end {
            return None;
        }

        let token: String = chars[start..end].iter().collect();
        if token.starts_with("http://") || token.starts_with("https://") {
            return None;
        }

        let clean_token = token.strip_prefix("file://").unwrap_or(&token);

        let mut parts = clean_token.split(':');
        let raw_path = parts.next()?;
        if raw_path.is_empty() {
            return None;
        }

        let line_num = parts.next().and_then(|p| p.parse::<usize>().ok());
        let col_num = parts.next().and_then(|p| p.parse::<usize>().ok());

        let path_obj = std::path::Path::new(raw_path);
        let exists = if path_obj.is_absolute() {
            path_obj.exists()
        } else if let Some(cwd) = &self.cwd {
            std::path::Path::new(cwd).join(path_obj).exists()
        } else {
            path_obj.exists()
        };

        if exists {
            Some(FileTarget {
                path: raw_path.to_string(),
                line: line_num,
                col: col_num,
            })
        } else {
            None
        }
    }

    /// Bir önceki komut bloğunun başladığı satıra kaydırır (Ctrl + Shift + Up)
    pub fn scroll_to_prev_command(&mut self) -> bool {
        let total_lines = self.scrollback.len() + self.rows;
        if total_lines == 0 {
            return false;
        }

        let current_top_vidx = self.scrollback.len().saturating_sub(self.scroll_offset);
        let mut target_vidx = None;
        let mut prev_block_id = None;

        for vidx in (0..current_top_vidx).rev() {
            let line_cells = if vidx < self.scrollback.len() {
                &self.scrollback[vidx]
            } else {
                let r = vidx - self.scrollback.len();
                let start = r * self.cols;
                let end = (start + self.cols).min(self.cells.len());
                &self.cells[start..end]
            };

            let bid = line_cells.iter().find_map(|c| c.block_id);
            if let Some(id) = bid {
                if prev_block_id.is_some() && prev_block_id != Some(id) {
                    target_vidx = Some(vidx + 1);
                    break;
                }
                prev_block_id = Some(id);
                target_vidx = Some(vidx);
            }
        }

        if let Some(target) = target_vidx {
            if target <= self.scrollback.len() {
                self.scroll_offset = self.scrollback.len() - target;
                self.mark_all_dirty();
                return true;
            }
        }
        false
    }

    /// Bir sonraki komut bloğunun başladığı satıra kaydırır (Ctrl + Shift + Down)
    pub fn scroll_to_next_command(&mut self) -> bool {
        let total_lines = self.scrollback.len() + self.rows;
        if total_lines == 0 || self.scroll_offset == 0 {
            return false;
        }

        let current_top_vidx = self.scrollback.len().saturating_sub(self.scroll_offset);
        let mut target_vidx = None;
        let mut current_block_id = None;

        if current_top_vidx < self.scrollback.len() {
            current_block_id = self.scrollback[current_top_vidx].iter().find_map(|c| c.block_id);
        }

        for vidx in (current_top_vidx + 1)..total_lines {
            let line_cells = if vidx < self.scrollback.len() {
                &self.scrollback[vidx]
            } else {
                let r = vidx - self.scrollback.len();
                let start = r * self.cols;
                let end = (start + self.cols).min(self.cells.len());
                &self.cells[start..end]
            };

            let bid = line_cells.iter().find_map(|c| c.block_id);
            if bid.is_some() && bid != current_block_id {
                target_vidx = Some(vidx);
                break;
            }
        }

        if let Some(target) = target_vidx {
            if target < self.scrollback.len() {
                self.scroll_offset = self.scrollback.len() - target;
            } else {
                self.scroll_offset = 0;
            }
        } else {
            self.scroll_offset = 0;
        }
        self.mark_all_dirty();
        true
    }

    /// Son çalıştırılan komutun terminal çıktısını getirir (OSC 133 blok tabanlı veya görünür son satırlar)
    pub fn get_last_command_output(&self) -> Option<String> {
        let last_block_id = self.blocks.last().map(|b| b.id).or(self.current_block_id);

        if let Some(target_id) = last_block_id {
            let mut output_lines = Vec::new();
            for row in &self.scrollback {
                if row.iter().any(|c| c.get_block_id() == Some(target_id) && c.is_output) {
                    let s: String = row
                        .iter()
                        .filter(|c| c.get_block_id() == Some(target_id) && c.is_output)
                        .map(|c| if c.c == '\0' { ' ' } else { c.c })
                        .collect();
                    let trimmed = s.trim_end();
                    if !trimmed.is_empty() {
                        output_lines.push(trimmed.to_string());
                    }
                }
            }
            for r in 0..self.rows {
                let start = r * self.cols;
                let end = (start + self.cols).min(self.cells.len());
                let row = &self.cells[start..end];
                if row.iter().any(|c| c.get_block_id() == Some(target_id) && c.is_output) {
                    let s: String = row
                        .iter()
                        .filter(|c| c.get_block_id() == Some(target_id) && c.is_output)
                        .map(|c| if c.c == '\0' { ' ' } else { c.c })
                        .collect();
                    let trimmed = s.trim_end();
                    if !trimmed.is_empty() {
                        output_lines.push(trimmed.to_string());
                    }
                }
            }
            if !output_lines.is_empty() {
                return Some(output_lines.join("\n"));
            } else {
                return None;
            }
        }

        // OSC 133 sinyali yoksa ekranın son dolu satırlarını döndür
        let mut lines = Vec::new();
        for r in 0..self.rows {
            let line_cells = self.get_display_line(r);
            let s: String = line_cells.iter().map(|c| if c.c == '\0' { ' ' } else { c.c }).collect();
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }
        if !lines.is_empty() {
            Some(lines.join("\n"))
        } else {
            None
        }
    }

    /// Çıktı içi arama sorgusunu ayarlar ve tüm geçmişi tarar
    pub fn set_search_query(&mut self, query: Option<String>) {
        let q = match query {
            Some(s) if !s.is_empty() => s,
            _ => {
                self.search_query = None;
                self.search_matches.clear();
                self.active_match_idx = 0;
                self.mark_all_dirty();
                return;
            }
        };

        let total_lines = self.scrollback.len() + self.rows;
        let mut matches = Vec::new();
        // NOT: to_lowercase().next() projeksiyonu 1:1 char eşlemesi varsayar.
        // "ß" -> "ss" gibi 1:N genişleyen karakterlerde arama eşleşmeyebilir;
        // bu bilinen bir sınırlandırmadır ve panic üretmez.
        let q_lower: Vec<char> = q.chars()
            .map(|c| c.to_lowercase().next().unwrap_or(c)).collect();

        if !q_lower.is_empty() {
            let q_len = q_lower.len();
            for vidx in 0..total_lines {
                let line_cells = if vidx < self.scrollback.len() {
                    &self.scrollback[vidx]
                } else {
                    let r = vidx - self.scrollback.len();
                    let start = r * self.cols;
                    let end = (start + self.cols).min(self.cells.len());
                    &self.cells[start..end]
                };
                let line_str: String = line_cells.iter().map(|c| c.c).collect();
                let line_chars: Vec<char> = line_str.chars().collect();
                let line_lower: Vec<char> = line_chars.iter()
                    .map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();

                if line_lower.len() < q_len {
                    continue;
                }

                for i in 0..=(line_lower.len() - q_len) {
                    if line_lower[i..i + q_len] == q_lower[..] {
                        matches.push(SearchMatch {
                            virtual_line: vidx,
                            start_col: i,
                            end_col: i + q_len,
                        });
                    }
                }
            }
        }

        self.search_query = Some(q);
        self.search_matches = matches;
        self.active_match_idx = 0;
        self.jump_to_active_match();
        self.mark_all_dirty();
    }

    pub fn jump_to_active_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let m = &self.search_matches[self.active_match_idx];
        if m.virtual_line < self.scrollback.len() {
            self.scroll_offset = self.scrollback.len() - m.virtual_line;
        } else {
            self.scroll_offset = 0;
        }
        self.mark_all_dirty();
    }

    pub fn next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.active_match_idx = (self.active_match_idx + 1) % self.search_matches.len();
        self.jump_to_active_match();
    }

    pub fn prev_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.active_match_idx == 0 {
            self.active_match_idx = self.search_matches.len() - 1;
        } else {
            self.active_match_idx -= 1;
        }
        self.jump_to_active_match();
    }

    /// Verilen koordinatın bir arama eşleşmesi olup olmadığını döner (Some(true) = aktif, Some(false) = diğer)
    pub fn get_search_match_status(&self, col: usize, screen_row: usize) -> Option<bool> {
        if self.search_matches.is_empty() {
            return None;
        }
        let vidx = if self.scroll_offset > 0 {
            let top_vidx = self.scrollback.len().saturating_sub(self.scroll_offset);
            top_vidx + screen_row
        } else {
            self.scrollback.len() + screen_row
        };

        for (idx, m) in self.search_matches.iter().enumerate() {
            if m.virtual_line == vidx && col >= m.start_col && col < m.end_col {
                return Some(idx == self.active_match_idx);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMatch {
    pub virtual_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTarget {
    pub path: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
}
