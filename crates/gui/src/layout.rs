#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal, // Yan yana iki panel (sol / sag)
    Vertical,   // Alt alta iki panel (ust / alt)
}

#[derive(Debug, Clone, PartialEq)]
pub enum TabTree {
    Single(usize),
    Split {
        direction: SplitDirection,
        first: Box<TabTree>,
        second: Box<TabTree>,
        ratio: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneGeometry {
    pub pane_id: usize,
    pub col_offset: usize,
    pub row_offset: usize,
    pub cols: usize,
    pub rows: usize,
}

impl PaneGeometry {
    pub fn contains(&self, col: usize, row: usize) -> bool {
        col >= self.col_offset
            && col < self.col_offset + self.cols
            && row >= self.row_offset
            && row < self.row_offset + self.rows
    }
}

impl TabTree {
    pub fn split_pane(
        &mut self,
        target_id: usize,
        new_pane_id: usize,
        direction: SplitDirection,
    ) -> bool {
        match self {
            TabTree::Single(id) if *id == target_id => {
                let first = Box::new(TabTree::Single(target_id));
                let second = Box::new(TabTree::Single(new_pane_id));
                *self = TabTree::Split {
                    direction,
                    first,
                    second,
                    ratio: 0.5,
                };
                true
            }
            TabTree::Single(_) => false,
            TabTree::Split { first, second, .. } => {
                if first.split_pane(target_id, new_pane_id, direction) {
                    true
                } else {
                    second.split_pane(target_id, new_pane_id, direction)
                }
            }
        }
    }

    pub fn remove_pane(&mut self, target_id: usize) -> Option<bool> {
        match self {
            TabTree::Single(id) if *id == target_id => None,
            TabTree::Single(_) => Some(false),
            TabTree::Split { first, second, .. } => {
                if let TabTree::Single(id) = **first {
                    if id == target_id {
                        *self = *second.clone();
                        return Some(true);
                    }
                }
                if let TabTree::Single(id) = **second {
                    if id == target_id {
                        *self = *first.clone();
                        return Some(true);
                    }
                }

                if first.remove_pane(target_id) == Some(true) {
                    Some(true)
                } else {
                    second.remove_pane(target_id)
                }
            }
        }
    }

    pub fn contains_pane(&self, target_id: usize) -> bool {
        match self {
            TabTree::Single(id) => *id == target_id,
            TabTree::Split { first, second, .. } => {
                first.contains_pane(target_id) || second.contains_pane(target_id)
            }
        }
    }

    pub fn adjust_pane_ratio(
        &mut self,
        pane_id: usize,
        dir: SplitDirection,
        delta: f32,
    ) -> bool {
        match self {
            TabTree::Single(_) => false,
            TabTree::Split {
                direction,
                first,
                second,
                ratio,
            } => {
                if first.contains_pane(pane_id) {
                    if first.adjust_pane_ratio(pane_id, dir, delta) {
                        return true;
                    }
                    if *direction == dir {
                        *ratio = (*ratio + delta).clamp(0.15, 0.85);
                        return true;
                    }
                } else if second.contains_pane(pane_id) {
                    if second.adjust_pane_ratio(pane_id, dir, delta) {
                        return true;
                    }
                    if *direction == dir {
                        *ratio = (*ratio - delta).clamp(0.15, 0.85);
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn calculate_layout(
        &self,
        x: usize,
        y: usize,
        cols: usize,
        rows: usize,
    ) -> Vec<PaneGeometry> {
        let mut geoms = Vec::new();
        self.collect_layout(x, y, cols, rows, &mut geoms);
        geoms
    }

    fn collect_layout(
        &self,
        x: usize,
        y: usize,
        cols: usize,
        rows: usize,
        out: &mut Vec<PaneGeometry>,
    ) {
        match self {
            TabTree::Single(id) => {
                out.push(PaneGeometry {
                    pane_id: *id,
                    col_offset: x,
                    row_offset: y,
                    cols: cols.max(1),
                    rows: rows.max(1),
                });
            }
            TabTree::Split {
                direction,
                first,
                second,
                ratio,
            } => match direction {
                SplitDirection::Horizontal => {
                    let (first_cols, second_cols) = split_dimension(cols, *ratio, 4);
                    first.collect_layout(x, y, first_cols, rows, out);
                    second.collect_layout(x + first_cols, y, second_cols, rows, out);
                }
                SplitDirection::Vertical => {
                    let (first_rows, second_rows) = split_dimension(rows, *ratio, 2);
                    first.collect_layout(x, y, cols, first_rows, out);
                    second.collect_layout(x, y + first_rows, cols, second_rows, out);
                }
            },
        }
    }

    pub fn collect_dividers(
        &self,
        x: usize,
        y: usize,
        cols: usize,
        rows: usize,
    ) -> Vec<TreeDivider> {
        let mut dividers = Vec::new();
        let mut split_counter = 0;
        self.collect_dividers_inner(x, y, cols, rows, &mut split_counter, &mut dividers);
        dividers
    }

    fn collect_dividers_inner(
        &self,
        x: usize,
        y: usize,
        cols: usize,
        rows: usize,
        split_counter: &mut usize,
        out: &mut Vec<TreeDivider>,
    ) {
        match self {
            TabTree::Single(_) => {}
            TabTree::Split {
                direction,
                first,
                second,
                ratio,
            } => {
                let this_index = *split_counter;
                *split_counter += 1;

                match direction {
                    SplitDirection::Horizontal => {
                        let (first_cols, second_cols) = split_dimension(cols, *ratio, 4);

                        out.push(TreeDivider {
                            split_index: this_index,
                            direction: SplitDirection::Horizontal,
                            col_or_row: x + first_cols,
                            span_start: y,
                            span_end: y + rows,
                            total_span: cols,
                            current_ratio: *ratio,
                        });

                        first.collect_dividers_inner(x, y, first_cols, rows, split_counter, out);
                        second.collect_dividers_inner(x + first_cols, y, second_cols, rows, split_counter, out);
                    }
                    SplitDirection::Vertical => {
                        let (first_rows, second_rows) = split_dimension(rows, *ratio, 2);

                        out.push(TreeDivider {
                            split_index: this_index,
                            direction: SplitDirection::Vertical,
                            col_or_row: y + first_rows,
                            span_start: x,
                            span_end: x + cols,
                            total_span: rows,
                            current_ratio: *ratio,
                        });

                        first.collect_dividers_inner(x, y, cols, first_rows, split_counter, out);
                        second.collect_dividers_inner(x, y + first_rows, cols, second_rows, split_counter, out);
                    }
                }
            }
        }
    }

    pub fn set_split_ratio_by_index(&mut self, target_index: usize, new_ratio: f32) -> bool {
        let mut split_counter = 0;
        self.set_split_ratio_inner(target_index, new_ratio, &mut split_counter)
    }

    fn set_split_ratio_inner(
        &mut self,
        target_index: usize,
        new_ratio: f32,
        split_counter: &mut usize,
    ) -> bool {
        match self {
            TabTree::Single(_) => false,
            TabTree::Split {
                first,
                second,
                ratio,
                ..
            } => {
                let this_index = *split_counter;
                *split_counter += 1;
                if this_index == target_index {
                    *ratio = new_ratio.clamp(0.15, 0.85);
                    true
                } else if first.set_split_ratio_inner(target_index, new_ratio, split_counter) {
                    true
                } else {
                    second.set_split_ratio_inner(target_index, new_ratio, split_counter)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeDivider {
    pub split_index: usize,
    pub direction: SplitDirection,
    pub col_or_row: usize,
    pub span_start: usize,
    pub span_end: usize,
    pub total_span: usize,
    pub current_ratio: f32,
}

fn split_dimension(total: usize, ratio: f32, min_margin: usize) -> (usize, usize) {
    if total <= min_margin * 2 {
        let first = (total / 2).max(1);
        (first, total.saturating_sub(first))
    } else {
        let min_val = min_margin;
        let max_val = total - min_margin;
        let desired = ((total as f32) * ratio).round() as usize;
        let first = desired.clamp(min_val, max_val);
        (first, total.saturating_sub(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_pane_layout() {
        let tree = TabTree::Single(1);
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].pane_id, 1);
        assert_eq!(layout[0].col_offset, 0);
        assert_eq!(layout[0].cols, 100);
        assert_eq!(layout[0].rows, 30);
    }

    #[test]
    fn test_horizontal_split() {
        let mut tree = TabTree::Single(1);
        assert!(tree.split_pane(1, 2, SplitDirection::Horizontal));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout.len(), 2);
        assert_eq!(layout[0].pane_id, 1);
        assert_eq!(layout[0].col_offset, 0);
        assert_eq!(layout[0].cols, 50);
        assert_eq!(layout[1].pane_id, 2);
        assert_eq!(layout[1].col_offset, 50);
        assert_eq!(layout[1].cols, 50);
    }

    #[test]
    fn test_vertical_split() {
        let mut tree = TabTree::Single(1);
        assert!(tree.split_pane(1, 2, SplitDirection::Vertical));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout.len(), 2);
        assert_eq!(layout[0].pane_id, 1);
        assert_eq!(layout[0].row_offset, 0);
        assert_eq!(layout[0].rows, 15);
        assert_eq!(layout[1].pane_id, 2);
        assert_eq!(layout[1].row_offset, 15);
        assert_eq!(layout[1].rows, 15);
    }

    #[test]
    fn test_remove_pane() {
        let mut tree = TabTree::Single(1);
        tree.split_pane(1, 2, SplitDirection::Horizontal);
        assert_eq!(tree.remove_pane(1), Some(true));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].pane_id, 2);
    }

    #[test]
    fn test_adjust_pane_ratio() {
        let mut tree = TabTree::Single(1);
        tree.split_pane(1, 2, SplitDirection::Horizontal);
        assert!(tree.adjust_pane_ratio(1, SplitDirection::Horizontal, 0.1));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout[0].cols, 60);
        assert_eq!(layout[1].cols, 40);

        // Küçültme testi
        assert!(tree.adjust_pane_ratio(1, SplitDirection::Horizontal, -0.2));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout[0].cols, 40);
        assert_eq!(layout[1].cols, 60);
    }

    #[test]
    fn test_collect_dividers_and_set_ratio() {
        let mut tree = TabTree::Single(1);
        tree.split_pane(1, 2, SplitDirection::Horizontal);
        let divs = tree.collect_dividers(0, 0, 100, 30);
        assert_eq!(divs.len(), 1);
        assert_eq!(divs[0].split_index, 0);
        assert_eq!(divs[0].direction, SplitDirection::Horizontal);
        assert_eq!(divs[0].col_or_row, 50);
        assert_eq!(divs[0].span_start, 0);
        assert_eq!(divs[0].span_end, 30);

        assert!(tree.set_split_ratio_by_index(0, 0.7));
        let layout = tree.calculate_layout(0, 0, 100, 30);
        assert_eq!(layout[0].cols, 70);
        assert_eq!(layout[1].cols, 30);
    }

    #[test]
    fn test_small_window_split_never_panics() {
        let mut tree = TabTree::Single(1);
        tree.split_pane(1, 2, SplitDirection::Horizontal);
        tree.split_pane(2, 3, SplitDirection::Vertical);
        // Aşırı küçük pencere boyutunda (min > max hatası veren cols = 3, rows = 2) asla panic atmamalı
        let layout = tree.calculate_layout(0, 0, 3, 2);
        assert_eq!(layout.len(), 3);
        let divs = tree.collect_dividers(0, 0, 3, 2);
        assert_eq!(divs.len(), 2);
    }
}
