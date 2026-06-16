use std::fmt;

/// Placement direction. Each variant maps to a (row_delta, col_delta).
///
/// Cardinal:
/// - `N`: (-1, 0)  — bottom to top (vertical backward)
/// - `S`: (1, 0)   — top to bottom (vertical)
/// - `E`: (0, 1)   — left to right (horizontal)
/// - `W`: (0, -1)  — right to left (horizontal backward)
///
/// Intercardinal:
/// - `NE`: (-1, 1) — bottom-left to top-right (diagonal backward)
/// - `NW`: (-1, -1) — bottom-right to top-left (diagonal backward)
/// - `SE`: (1, 1)  — top-left to bottom-right (diagonal)
/// - `SW`: (1, -1) — top-right to bottom-left (diagonal backward)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Direction {
    pub fn delta(&self) -> (isize, isize) {
        match self {
            Direction::N => (-1, 0),
            Direction::NE => (-1, 1),
            Direction::E => (0, 1),
            Direction::SE => (1, 1),
            Direction::S => (1, 0),
            Direction::SW => (1, -1),
            Direction::W => (0, -1),
            Direction::NW => (-1, -1),
        }
    }

    pub fn all() -> [Direction; 8] {
        [
            Direction::N,
            Direction::NE,
            Direction::E,
            Direction::SE,
            Direction::S,
            Direction::SW,
            Direction::W,
            Direction::NW,
        ]
    }

    pub fn has_horizontal(&self) -> bool {
        matches!(
            self,
            Self::E | Self::W | Self::NE | Self::NW | Self::SE | Self::SW
        )
    }

    pub fn has_vertical(&self) -> bool {
        matches!(
            self,
            Self::N | Self::S | Self::NE | Self::NW | Self::SE | Self::SW
        )
    }
}

/// Dimensions for manual grid sizing.
#[derive(Debug, Clone)]
pub struct GridSize {
    pub width: usize,
    pub height: usize,
}

/// A hidden word candidate with an accompanying hint.
#[derive(Debug, Clone)]
pub struct HiddenWord {
    pub word: String,
    pub hint: String,
}

/// Configuration for puzzle generation.
///
/// `size` controls the grid dimensions:
/// - `None` — auto-calculated to fit all visible words plus the selected hidden word
/// - `Some(GridSize)` — uses the provided dimensions; validation ensures enough cells
///
/// `max_word_attempts` controls how many random placements are tried per word before giving up
/// on the current grid. `max_grid_attempts` controls how many times the entire grid is retried
/// when a word cannot be placed.
#[derive(Debug, Clone)]
pub struct PuzzleConfig {
    pub size: Option<GridSize>,
    pub directions: Vec<Direction>,
    pub max_word_attempts: usize,
    pub max_grid_attempts: usize,
    pub hidden_words: Vec<HiddenWord>,
}

/// A placed word and its position within the grid.
#[derive(Debug, Clone)]
pub struct WordPlacement {
    pub word: String,
    pub row: usize,
    pub col: usize,
    pub direction: Direction,
}

/// A generated word search puzzle.
#[derive(Debug, Clone)]
pub struct Puzzle {
    pub grid: Vec<Vec<char>>,
    pub placements: Vec<WordPlacement>,
    pub hidden_word: HiddenWord,
}

impl Puzzle {
    pub fn words(&self) -> Vec<&str> {
        self.placements.iter().map(|p| p.word.as_str()).collect()
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.grid.len(), self.grid.first().map_or(0, |r| r.len()))
    }
}

/// Generates a word search puzzle.
///
/// Places each visible word in a grid using the allowed directions, then fills
/// remaining cells with the selected hidden word's letters (left-to-right,
/// top-to-bottom). If any word cannot be placed, the entire grid is retried
/// up to `max_grid_attempts` times before returning an error.
///
/// # Errors
///
/// Returns an error if:
/// - The hidden words list is empty.
/// - Any word (visible or hidden) is empty or whitespace-only.
/// - No valid visible words remain after filtering (hidden word excluded, duplicates removed).
/// - A manual grid is too small to fit all words.
/// - No directions are specified.
pub fn generate(words: &[String], config: &PuzzleConfig) -> Result<Puzzle, PuzzleError> {
    if config.hidden_words.is_empty() {
        return Err(PuzzleError::NoHiddenWords);
    }

    if config.directions.is_empty() {
        return Err(PuzzleError::NoDirections);
    }

    for hw in &config.hidden_words {
        if hw.word.trim().is_empty() {
            return Err(PuzzleError::EmptyWord {
                word: hw.word.clone(),
            });
        }
    }

    for word in words {
        if word.trim().is_empty() {
            return Err(PuzzleError::EmptyWord { word: word.clone() });
        }
    }

    let hidden_idx = fastrand::usize(0..config.hidden_words.len());
    let selected = &config.hidden_words[hidden_idx];
    let hidden_upper = selected.word.to_uppercase();

    let visible = {
        let mut seen = std::collections::HashSet::new();
        words
            .iter()
            .filter(|w| {
                let upper = w.to_uppercase();
                upper != hidden_upper && seen.insert(upper)
            })
            .collect::<Vec<&String>>()
    };

    if visible.is_empty() {
        return Err(PuzzleError::EmptyWordList);
    }

    let total_visible: usize = visible.iter().map(|w| w.chars().count()).sum();
    let total = total_visible + hidden_upper.chars().count();

    let longest = visible.iter().map(|w| w.chars().count()).max().unwrap();
    let (min_width, min_height) = compute_min_bounds(longest, &config.directions);

    let (width, height) = match &config.size {
        Some(g) => {
            if g.width * g.height < total {
                return Err(PuzzleError::InvalidGridSize);
            }
            (g.width, g.height)
        }
        None => compute_grid_size(total, min_width, min_height),
    };

    for _ in 0..config.max_grid_attempts {
        let mut grid = vec![vec!['\0'; width]; height];
        let mut placements = Vec::new();
        let mut all_placed = true;

        let mut indices: Vec<usize> = (0..visible.len()).collect();
        indices.sort_by(|&a, &b| visible[b].chars().count().cmp(&visible[a].chars().count()));

        for &idx in &indices {
            let word = visible[idx];
            let chars: Vec<char> = word.to_uppercase().chars().collect();
            let len = chars.len();

            let mut placed = false;

            for _ in 0..config.max_word_attempts {
                let dir_idx = fastrand::usize(0..config.directions.len());
                let dir = config.directions[dir_idx];
                let (dr, dc) = dir.delta();

                let row_fits = if dr != 0 { height >= len } else { true };
                let col_fits = if dc != 0 { width >= len } else { true };
                if !row_fits || !col_fits {
                    continue;
                }

                let (min_row, max_row) = direction_bounds(len, height - 1, dr);
                if min_row > max_row {
                    continue;
                }

                let (min_col, max_col) = direction_bounds(len, width - 1, dc);
                if min_col > max_col {
                    continue;
                }

                let start_row = fastrand::usize(min_row..=max_row);
                let start_col = fastrand::usize(min_col..=max_col);

                let mut fits = true;
                for ((r, c), &_ch) in positions(start_row, start_col, dr, dc, len).zip(chars.iter())
                {
                    let cell = grid[r][c];
                    if cell != '\0' {
                        fits = false;
                        break;
                    }
                }

                if fits {
                    for ((r, c), &ch) in
                        positions(start_row, start_col, dr, dc, len).zip(chars.iter())
                    {
                        grid[r][c] = ch;
                    }
                    placements.push(WordPlacement {
                        word: word.clone(),
                        row: start_row,
                        col: start_col,
                        direction: dir,
                    });
                    placed = true;
                    break;
                }
            }

            if !placed {
                all_placed = false;
                break;
            }
        }

        if all_placed {
            let hidden_chars: Vec<char> = hidden_upper.chars().collect();
            let mut hid_idx = 0;
            for row in grid.iter_mut() {
                for cell in row.iter_mut() {
                    if *cell == '\0' {
                        *cell = hidden_chars[hid_idx % hidden_chars.len()];
                        hid_idx += 1;
                    }
                }
            }

            return Ok(Puzzle {
                grid,
                placements,
                hidden_word: HiddenWord {
                    word: selected.word.clone(),
                    hint: selected.hint.clone(),
                },
            });
        }
    }

    Err(PuzzleError::PlacementFailed)
}

fn compute_min_bounds(longest_word: usize, directions: &[Direction]) -> (usize, usize) {
    let has_h = directions.iter().any(|d| d.has_horizontal());
    let has_v = directions.iter().any(|d| d.has_vertical());
    let min_w = if has_h { longest_word } else { 1 };
    let min_h = if has_v { longest_word } else { 1 };
    (min_w, min_h)
}

fn compute_grid_size(total: usize, min_width: usize, min_height: usize) -> (usize, usize) {
    let limit = total.min(10000);

    // First: find exact-fit grids (w * h == total)
    let mut best_exact = None;
    let mut best_exact_diff = usize::MAX;

    for w in min_width..=limit {
        if total.is_multiple_of(w) {
            let h = total / w;
            if h >= min_height {
                let diff = w.abs_diff(h);
                if diff < best_exact_diff {
                    best_exact = Some((w, h));
                    best_exact_diff = diff;
                }
            }
        }
    }

    if let Some(g) = best_exact {
        return g;
    }

    // Fallback: minimal-padding grid
    let mut best = (1, total);
    let mut best_padding = usize::MAX;
    let mut best_diff = usize::MAX;

    for w in min_width..=limit {
        let h = total.div_ceil(w).max(min_height);
        let padding = w * h - total;
        let diff = w.abs_diff(h);

        if padding < best_padding || (padding == best_padding && diff < best_diff) {
            best = (w, h);
            best_padding = padding;
            best_diff = diff;
        }
    }

    best
}

fn direction_bounds(len: usize, max_idx: usize, delta: isize) -> (usize, usize) {
    if delta > 0 {
        (0, max_idx + 1 - len)
    } else if delta < 0 {
        (len - 1, max_idx)
    } else {
        (0, max_idx)
    }
}

fn positions(
    row: usize,
    col: usize,
    dr: isize,
    dc: isize,
    len: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (0..len).map(move |i| {
        let r = (row as isize + i as isize * dr) as usize;
        let c = (col as isize + i as isize * dc) as usize;
        (r, c)
    })
}

#[derive(Debug, Clone)]
pub enum PuzzleError {
    EmptyWordList,
    PlacementFailed,
    EmptyWord { word: String },
    NoHiddenWords,
    InvalidGridSize,
    NoDirections,
}

impl fmt::Display for PuzzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PuzzleError::EmptyWordList => write!(f, "no valid visible words to place"),
            PuzzleError::PlacementFailed => {
                write!(f, "failed to place all words after all attempts")
            }
            PuzzleError::EmptyWord { word } => {
                write!(f, "word '{word}' is empty or whitespace-only")
            }
            PuzzleError::NoHiddenWords => write!(f, "at least one hidden word must be provided"),
            PuzzleError::InvalidGridSize => write!(
                f,
                "manual grid is too small for all visible words and hidden word"
            ),
            PuzzleError::NoDirections => write!(f, "at least one direction must be specified"),
        }
    }
}

impl std::error::Error for PuzzleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn default_config() -> PuzzleConfig {
        PuzzleConfig {
            size: None,
            directions: Direction::all().to_vec(),
            max_word_attempts: 1000,
            max_grid_attempts: 100,
            hidden_words: vec![],
        }
    }

    fn solve(
        grid: &[Vec<char>],
        placements: &[WordPlacement],
        visible_words: &[String],
        hidden_word: &str,
    ) -> Result<(), Vec<String>> {
        let height = grid.len();
        let width = grid.first().map_or(0, |r| r.len());
        let mut errors = Vec::new();

        'words: for word in visible_words {
            let upper = word.to_uppercase();
            let chars: Vec<char> = upper.chars().collect();
            let len = chars.len();

            for row in 0..height {
                for col in 0..width {
                    for dir in Direction::all() {
                        let (dr, dc) = dir.delta();

                        let end_row = row as isize + (len as isize - 1) * dr;
                        let end_col = col as isize + (len as isize - 1) * dc;
                        if end_row < 0 || end_row >= height as isize {
                            continue;
                        }
                        if end_col < 0 || end_col >= width as isize {
                            continue;
                        }

                        let mut found = true;
                        for ((r, c), &ch) in positions(row, col, dr, dc, len).zip(chars.iter()) {
                            if grid[r][c] != ch {
                                found = false;
                                break;
                            }
                        }

                        if found {
                            continue 'words;
                        }
                    }
                }
            }

            errors.push(word.clone());
        }

        let mut used = vec![vec![false; width]; height];
        for p in placements {
            let (dr, dc) = p.direction.delta();
            for (r, c) in positions(p.row, p.col, dr, dc, p.word.chars().count()) {
                used[r][c] = true;
            }
        }

        let hidden_upper = hidden_word.to_uppercase();
        let hidden_chars: Vec<char> = hidden_upper.chars().collect();
        let mut leftover = String::new();
        for row in 0..height {
            for col in 0..width {
                if !used[row][col] {
                    leftover.push(grid[row][col]);
                }
            }
        }

        let matches_hidden = hidden_chars.is_empty()
            || leftover
                .chars()
                .enumerate()
                .all(|(i, c)| c == hidden_chars[i % hidden_chars.len()]);

        if !matches_hidden {
            let truncated: String = leftover.chars().take(20).collect();
            errors.push(format!(
                "hidden word mismatch: leftover starts with '{}', expected pattern '{}'",
                truncated, hidden_upper
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[test]
    fn basic_generation() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "fire".into(),
                hint: "element".into(),
            }],
            ..default_config()
        };
        let words = vec!["rust".into(), "python".into(), "java".into()];
        let puzzle = generate(&words, &config).unwrap();

        assert_eq!(puzzle.hidden_word.word, "fire");
        assert_eq!(puzzle.hidden_word.hint, "element");
        assert_eq!(puzzle.placements.len(), 3);
        assert!(puzzle.words().contains(&"rust"));
        assert!(puzzle.words().contains(&"python"));
        assert!(puzzle.words().contains(&"java"));

        let (h, w) = puzzle.dimensions();
        let total_visible: usize = words.iter().map(|word| word.chars().count()).sum();
        let total = total_visible + "fire".len();
        assert!(
            h * w >= total,
            "grid must fit at least {} cells, got {}",
            total,
            h * w
        );

        assert!(solve(&puzzle.grid, &puzzle.placements, &words, "fire").is_ok());
    }

    #[test]
    fn solver_detects_corrupted_visible() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "fire".into(),
                hint: "element".into(),
            }],
            ..default_config()
        };
        let words = vec!["rust".into(), "python".into()];
        let puzzle = generate(&words, &config).unwrap();

        let mut corrupted = puzzle.grid.clone();
        for p in &puzzle.placements {
            let (dr, dc) = p.direction.delta();
            for (r, c) in positions(p.row, p.col, dr, dc, p.word.chars().count()) {
                corrupted[r][c] = 'X';
            }
        }

        let result = solve(&corrupted, &puzzle.placements, &words, "fire");
        assert!(result.is_err());
    }

    #[test]
    fn solver_detects_corrupted_hidden() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "fire".into(),
                hint: "element".into(),
            }],
            ..default_config()
        };
        let words = vec!["rust".into()];
        let puzzle = generate(&words, &config).unwrap();

        let height = puzzle.grid.len();
        let width = puzzle.grid[0].len();
        let mut used = vec![vec![false; width]; height];
        for p in &puzzle.placements {
            let (dr, dc) = p.direction.delta();
            for (r, c) in positions(p.row, p.col, dr, dc, p.word.chars().count()) {
                used[r][c] = true;
            }
        }

        let mut corrupted = puzzle.grid.clone();
        'outer: for r in 0..height {
            for c in 0..width {
                if !used[r][c] {
                    corrupted[r][c] = 'X';
                    break 'outer;
                }
            }
        }

        let result = solve(&corrupted, &puzzle.placements, &words, "fire");
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("hidden word")));
    }

    #[test]
    fn rejects_no_hidden_words() {
        let config = default_config();
        let words = vec!["hello".into()];
        let result = generate(&words, &config);
        assert!(matches!(result, Err(PuzzleError::NoHiddenWords)));
    }

    #[test]
    fn rejects_empty_hidden_word() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "".into(),
                hint: "empty".into(),
            }],
            ..default_config()
        };
        let words = vec!["hello".into()];
        let result = generate(&words, &config);
        assert!(matches!(result, Err(PuzzleError::EmptyWord { .. })));
    }

    #[test]
    fn rejects_empty_visible_word() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "secret".into(),
                hint: "hidden".into(),
            }],
            ..default_config()
        };
        let words = vec!["hello".into(), "".into()];
        let result = generate(&words, &config);
        assert!(matches!(result, Err(PuzzleError::EmptyWord { .. })));
    }

    #[test]
    fn rejects_manual_grid_too_small() {
        let config = PuzzleConfig {
            size: Some(GridSize {
                width: 4,
                height: 5,
            }),
            hidden_words: vec![HiddenWord {
                word: "secret".into(),
                hint: "hidden".into(),
            }],
            ..default_config()
        };
        let words = vec!["hello".into(), "world".into(), "puzzle".into()];
        let result = generate(&words, &config);
        assert!(matches!(result, Err(PuzzleError::InvalidGridSize)));
    }

    #[test]
    fn duplicate_is_filtered() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "rust".into(),
                hint: "language".into(),
            }],
            ..default_config()
        };
        let words = vec!["rust".into(), "python".into(), "java".into()];
        let puzzle = generate(&words, &config).unwrap();
        assert_eq!(puzzle.placements.len(), 2);
        assert!(!puzzle.words().contains(&"rust"));
    }

    #[test]
    fn rejects_no_directions() {
        let config = PuzzleConfig {
            directions: vec![],
            hidden_words: vec![HiddenWord {
                word: "secret".into(),
                hint: "hidden".into(),
            }],
            ..default_config()
        };
        let words = vec!["hello".into()];
        let result = generate(&words, &config);
        assert!(matches!(result, Err(PuzzleError::NoDirections)));
    }

    #[test]
    fn word_method() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "cat".into(),
                hint: "animal".into(),
            }],
            ..default_config()
        };
        let words = vec!["dog".into(), "bird".into()];
        let puzzle = generate(&words, &config).unwrap();
        let word_list = puzzle.words();
        assert_eq!(word_list.len(), 2);
        assert!(word_list.contains(&"dog"));
        assert!(word_list.contains(&"bird"));
    }

    #[test]
    fn auto_size_exact_fit() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "hi".into(),
                hint: "greeting".into(),
            }],
            ..default_config()
        };
        let words = vec!["abc".into(), "de".into()];
        let puzzle = generate(&words, &config).unwrap();
        let (h, w) = puzzle.dimensions();
        let total: usize = words.iter().map(|word| word.chars().count()).sum();
        let expected = total + 2;
        assert!(
            h * w >= expected,
            "grid must fit at least {} cells, got {}",
            expected,
            h * w
        );
    }

    #[test]
    fn grid_only_contains_allowed_letters() {
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "fire".into(),
                hint: "element".into(),
            }],
            ..default_config()
        };
        let words = vec!["rust".into(), "python".into(), "java".into()];
        let puzzle = generate(&words, &config).unwrap();

        let hidden_upper = puzzle.hidden_word.word.to_uppercase();
        let mut allowed: HashSet<char> = hidden_upper.chars().collect();
        for w in &words {
            allowed.extend(w.to_uppercase().chars());
        }

        for (r, row) in puzzle.grid.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                assert!(
                    allowed.contains(&ch),
                    "cell ({},{}) contains '{}' which is not in any visible word or the hidden word",
                    r,
                    c,
                    ch
                );
            }
        }
    }

    #[test]
    fn grid_is_tight_fit() {
        // 4 words × 3 chars + 2-char hidden = 14 total
        // min_w = 3, min_h = 3 (from all directions)
        // 3×5 = 15, padding = 1 < 2
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "xy".into(),
                hint: "test".into(),
            }],
            ..default_config()
        };
        let words = vec!["abc".into(), "def".into(), "ghi".into(), "jkl".into()];
        let puzzle = generate(&words, &config).unwrap();

        let (h, w) = puzzle.dimensions();
        let total_cells = h * w;
        let hidden_len = puzzle.hidden_word.word.len();
        let visible_chars: usize = words.iter().map(|w| w.chars().count()).sum();
        let needed = visible_chars + hidden_len;
        let padding = total_cells - needed;

        assert!(
            total_cells >= needed,
            "grid is too small: {} cells for {} needed",
            total_cells,
            needed
        );
        assert!(
            padding < hidden_len,
            "padding too large: {padding} cells, hidden word is {hidden_len} chars"
        );
    }

    #[test]
    fn leftover_spells_hidden_word_once() {
        // 3 words × 3 chars + 2-char hidden = 11 total
        // min_w = 3, min_h = 3, 3×4 = 12, padding = 1 < 2
        let config = PuzzleConfig {
            hidden_words: vec![HiddenWord {
                word: "xy".into(),
                hint: "test".into(),
            }],
            ..default_config()
        };
        let words = vec!["abc".into(), "def".into(), "ghi".into()];
        let puzzle = generate(&words, &config).unwrap();

        let height = puzzle.grid.len();
        let width = puzzle.grid[0].len();

        let mut used = vec![vec![false; width]; height];
        for p in &puzzle.placements {
            let (dr, dc) = p.direction.delta();
            for (r, c) in positions(p.row, p.col, dr, dc, p.word.chars().count()) {
                used[r][c] = true;
            }
        }

        let leftover: String = (0..height)
            .flat_map(|r| (0..width).map(move |c| (r, c)))
            .filter(|&(r, c)| !used[r][c])
            .map(|(r, c)| puzzle.grid[r][c])
            .collect();

        let hidden_upper = puzzle.hidden_word.word.to_uppercase();

        assert!(
            leftover.starts_with(&hidden_upper),
            "leftover '{leftover}' does not start with hidden word '{hidden_upper}'"
        );

        assert!(
            leftover.len() < hidden_upper.len() * 2,
            "leftover is {} chars but hidden word is {} chars (would repeat)",
            leftover.len(),
            hidden_upper.len()
        );
    }
}
