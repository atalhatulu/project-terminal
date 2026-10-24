#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub start_col: usize,
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl Selection {
    pub fn new(start_col: usize, start_row: usize, end_col: usize, end_row: usize) -> Self {
        Self {
            start_col,
            start_row,
            end_col,
            end_row,
        }
    }

    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            (
                (self.start_col, self.start_row),
                (self.end_col, self.end_row),
            )
        } else {
            (
                (self.end_col, self.end_row),
                (self.start_col, self.start_row),
            )
        }
    }

    pub fn contains(&self, col: usize, row: usize) -> bool {
        let ((s_col, s_row), (e_col, e_row)) = self.normalized();
        if row < s_row || row > e_row {
            return false;
        }
        if s_row == e_row {
            return col >= s_col && col <= e_col;
        }
        if row == s_row {
            return col >= s_col;
        }
        if row == e_row {
            return col <= e_col;
        }
        true
    }
}
