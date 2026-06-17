use std::collections::HashSet;
use std::fmt;

/// Configuration for puzzle generation.
///
/// `word_count` visible words are randomly selected from `word_dictionary`.
/// One hidden word is randomly selected from `solution_dictionary`.
/// Grid dimensions are computed based on `orientation` and the total
/// character count of all visible words plus the hidden word.
#[derive(Debug, Clone)]
pub struct PuzzleConfig {
    pub word_dictionary: Vec<String>,
    pub solution_dictionary: Vec<HiddenWord>,
    pub word_count: usize,
    pub orientation: Orientation,
    pub directions: DirectionConfig,
    pub max_word_attempts: usize,
    pub max_grid_attempts: usize,
}

impl PuzzleConfig {
    /// Generates a word search puzzle.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The solution or word dictionary is empty.
    /// - `word_count` is 0 or exceeds the word dictionary size.
    /// - No directions are enabled.
    /// - Any word (visible or hidden) is empty or whitespace-only.
    /// - A valid grid cannot be generated within the configured attempt limit.
    pub fn generate(&self) -> Result<Puzzle, PuzzleError> {
        if self.solution_dictionary.is_empty() {
            return Err(PuzzleError::NoSolutionWords);
        }
        if self.word_dictionary.is_empty() {
            return Err(PuzzleError::EmptyWordDictionary);
        }
        if self.word_count == 0 || self.word_count > self.word_dictionary.len() {
            return Err(PuzzleError::InvalidWordCount);
        }

        let directions = self.directions.to_directions();
        if directions.is_empty() {
            return Err(PuzzleError::NoDirections);
        }

        for hw in &self.solution_dictionary {
            if hw.word.trim().is_empty() {
                return Err(PuzzleError::EmptyWord {
                    word: hw.word.clone(),
                });
            }
        }
        for word in &self.word_dictionary {
            if word.trim().is_empty() {
                return Err(PuzzleError::EmptyWord { word: word.clone() });
            }
        }

        for _ in 0..self.max_grid_attempts {
            let hidden_idx = fastrand::usize(0..self.solution_dictionary.len());
            let selected = &self.solution_dictionary[hidden_idx];
            let hidden_upper = selected.word.to_uppercase();

            let visible =
                select_visible_words(&self.word_dictionary, self.word_count, &hidden_upper);
            if visible.len() < self.word_count {
                continue;
            }

            let visible_chars: usize = visible.iter().map(|w| w.len()).sum();
            let total = visible_chars + hidden_upper.len();
            let longest = visible.iter().map(|w| w.len()).max().unwrap();

            let (width, height) = compute_grid_size(total, longest, &directions, self.orientation);

            let mut grid = vec![vec!['\0'; width]; height];
            let mut placements = Vec::new();
            let mut all_placed = true;

            let mut sorted: Vec<&String> = visible.iter().collect();
            sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));

            for word in &sorted {
                let chars: Vec<char> = word.to_uppercase().chars().collect();
                let len = chars.len();
                let mut placed = false;

                let mut dir_shuffle: Vec<&Direction> = directions.iter().collect();
                fastrand::shuffle(&mut dir_shuffle);

                for dir in dir_shuffle {
                    let (dr, dc) = dir.delta();

                    let (min_row, max_row) = direction_bounds(len, height - 1, dr);
                    if min_row > max_row {
                        continue;
                    }
                    let (min_col, max_col) = direction_bounds(len, width - 1, dc);
                    if min_col > max_col {
                        continue;
                    }

                    for _ in 0..self.max_word_attempts.max(1).div_ceil(directions.len()) {
                        let start_row = fastrand::usize(min_row..=max_row);
                        let start_col = fastrand::usize(min_col..=max_col);

                        let mut fits = true;
                        for ((r, c), &ch) in
                            positions(start_row, start_col, dr, dc, len).zip(chars.iter())
                        {
                            let cell = grid[r][c];
                            if cell != '\0' && cell != ch {
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
                                word: (*word).clone(),
                                row: start_row,
                                col: start_col,
                                direction: *dir,
                            });
                            placed = true;
                            break;
                        }
                    }

                    if placed {
                        break;
                    }
                }

                if !placed {
                    all_placed = false;
                    break;
                }
            }

            if all_placed {
                let empty_count = grid.iter().flatten().filter(|&&c| c == '\0').count();

                if empty_count == hidden_upper.chars().count() {
                    fill_scrambled(&mut grid, &hidden_upper);

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
        }

        Err(PuzzleError::PlacementFailed)
    }
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

/// Placement direction.
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
}

/// Which word directions are allowed.
#[derive(Debug, Clone)]
pub struct DirectionConfig {
    pub horizontally: bool,
    pub vertically: bool,
    pub diagonally: bool,
    pub backward: bool,
}

impl DirectionConfig {
    /// Resolve the set of enabled [`Direction`] variants.
    ///
    /// Mapping:
    /// - `horizontally` → `E`
    /// - `vertically` → `S`
    /// - `diagonally` → `SE`, `SW`
    /// - `backward` → adds `W` (reverse of E), `N` (reverse of S),
    ///   `NW` (reverse of SE), `NE` (reverse of SW)
    ///   for each respective forward direction that is enabled.
    pub fn to_directions(&self) -> Vec<Direction> {
        let mut dirs = Vec::new();
        if self.horizontally {
            dirs.push(Direction::E);
            if self.backward {
                dirs.push(Direction::W);
            }
        }
        if self.vertically {
            dirs.push(Direction::S);
            if self.backward {
                dirs.push(Direction::N);
            }
        }
        if self.diagonally {
            dirs.push(Direction::SE);
            dirs.push(Direction::SW);
            if self.backward {
                dirs.push(Direction::NW);
                dirs.push(Direction::NE);
            }
        }
        dirs
    }
}

/// Grid shape preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Square,
    Landscape,
    Portrait,
}

/// A hidden word candidate with an accompanying hint.
#[derive(Debug, Clone)]
pub struct HiddenWord {
    pub word: String,
    pub hint: String,
}

/// A placed word and its position within the grid.
#[derive(Debug, Clone)]
pub struct WordPlacement {
    pub word: String,
    pub row: usize,
    pub col: usize,
    pub direction: Direction,
}

fn select_visible_words(dictionary: &[String], count: usize, hidden: &str) -> Vec<String> {
    let mut candidates: Vec<&String> = dictionary
        .iter()
        .filter(|w| w.to_uppercase() != hidden)
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }

    fastrand::shuffle(&mut candidates);
    let mut selected = Vec::new();
    let mut seen = HashSet::new();

    for word in candidates {
        if seen.insert(word.to_uppercase()) {
            selected.push(word.clone());
            if selected.len() == count {
                break;
            }
        }
    }

    selected
}

fn compute_grid_size(
    total: usize,
    longest: usize,
    directions: &[Direction],
    orientation: Orientation,
) -> (usize, usize) {
    let needs_width = directions.iter().any(|d| d.delta().1 != 0);
    let needs_height = directions.iter().any(|d| d.delta().0 != 0);
    let has_diagonal = directions
        .iter()
        .any(|d| d.delta().0 != 0 && d.delta().1 != 0);
    let has_non_diagonal = directions
        .iter()
        .any(|d| d.delta().0 == 0 || d.delta().1 == 0);

    let only_horizontal = needs_width && !needs_height && !has_diagonal;
    let only_vertical = !needs_width && needs_height && !has_diagonal;
    let only_diagonal = !has_non_diagonal && has_diagonal;

    let dim_ok = |wi: usize, hi: usize| -> bool {
        if only_horizontal && wi < longest {
            return false;
        }
        if only_vertical && hi < longest {
            return false;
        }
        if only_diagonal && (wi < longest || hi < longest) {
            return false;
        }
        if !only_horizontal && !only_vertical && !only_diagonal && wi < longest && hi < longest {
            return false;
        }
        true
    };

    let orient_ok = |wi: usize, hi: usize| -> bool {
        matches!(
            (orientation, wi >= hi),
            (Orientation::Landscape, true)
                | (Orientation::Portrait, false)
                | (Orientation::Square, _)
        )
    };

    let weight = |wi: usize, hi: usize| -> usize { wi.abs_diff(hi) };
    let max_pad = longest.max(3);
    let mut candidates = Vec::new();

    for pad in 0..=max_pad {
        let target = total + pad;
        let limit = (target as f64).sqrt() as usize;
        for w in 1..=limit {
            if target.is_multiple_of(w) {
                let h = target / w;
                for &(wi, hi) in &[(w, h), (h, w)] {
                    if dim_ok(wi, hi) {
                        candidates.push((wi, hi, pad));
                    }
                }
            }
        }
    }

    candidates.sort_by(|&(wi1, hi1, pad1), &(wi2, hi2, pad2)| {
        pad1.cmp(&pad2)
            .then_with(|| {
                let w1 = weight(wi1, hi1);
                let w2 = weight(wi2, hi2);
                w1.cmp(&w2)
            })
            .then_with(|| {
                let o1 = orient_ok(wi1, hi1);
                let o2 = orient_ok(wi2, hi2);
                o2.cmp(&o1)
            })
    });

    if let Some(&(w, h, _)) = candidates.first() {
        return (w, h);
    }

    let w = if needs_width { total.max(longest) } else { 1 };
    let h = if needs_height { total.max(longest) } else { 1 };
    (w, h)
}

fn direction_bounds(len: usize, max_idx: usize, delta: isize) -> (usize, usize) {
    if delta > 0 {
        if len > max_idx + 1 {
            return (1, 0);
        }
        (0, max_idx + 1 - len)
    } else if delta < 0 {
        if len > max_idx + 1 {
            return (1, 0);
        }
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

fn fill_scrambled(grid: &mut [Vec<char>], hidden: &str) {
    let mut chars: Vec<char> = hidden.chars().collect();
    fastrand::shuffle(&mut chars);
    let mut idx = 0;
    for row in grid.iter_mut() {
        for cell in row.iter_mut() {
            if *cell == '\0' {
                *cell = chars[idx];
                idx += 1;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PuzzleError {
    PlacementFailed,
    EmptyWord { word: String },
    NoSolutionWords,
    EmptyWordDictionary,
    InvalidWordCount,
    NoDirections,
}

impl fmt::Display for PuzzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PuzzleError::PlacementFailed => {
                write!(f, "failed to place all words after all attempts")
            }
            PuzzleError::EmptyWord { word } => {
                write!(f, "word '{word}' is empty or whitespace-only")
            }
            PuzzleError::NoSolutionWords => {
                write!(f, "at least one solution word must be provided")
            }
            PuzzleError::EmptyWordDictionary => {
                write!(f, "word dictionary is empty")
            }
            PuzzleError::InvalidWordCount => {
                write!(
                    f,
                    "word count must be between 1 and the size of the word dictionary"
                )
            }
            PuzzleError::NoDirections => write!(f, "at least one direction must be specified"),
        }
    }
}

impl std::error::Error for PuzzleError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn hidden(word: &str, hint: &str) -> Vec<HiddenWord> {
        vec![HiddenWord {
            word: word.into(),
            hint: hint.into(),
        }]
    }

    fn default_config() -> PuzzleConfig {
        PuzzleConfig {
            word_dictionary: vec![],
            solution_dictionary: vec![],
            word_count: 0,
            orientation: Orientation::Square,
            directions: DirectionConfig {
                horizontally: true,
                vertically: true,
                diagonally: true,
                backward: true,
            },
            max_word_attempts: 1000,
            max_grid_attempts: 100,
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
        let mut hidden_chars: Vec<char> = hidden_upper.chars().collect();
        hidden_chars.sort();
        let mut leftover = String::new();
        for row in 0..height {
            for col in 0..width {
                if !used[row][col] {
                    leftover.push(grid[row][col]);
                }
            }
        }
        let mut leftover_chars: Vec<char> = leftover.chars().collect();
        leftover_chars.sort();

        if leftover_chars != hidden_chars {
            errors.push(format!(
                "hidden word mismatch: leftover '{leftover}' does not match '{hidden_upper}'"
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
            word_dictionary: words(&["rust", "python", "java", "go", "c"]),
            solution_dictionary: hidden("fire", "element"),
            word_count: 3,
            ..default_config()
        };
        let puzzle = config.generate().unwrap();
        assert_eq!(puzzle.hidden_word.hint, "element");
        assert_eq!(puzzle.placements.len(), 3);
        assert!(!puzzle.words().contains(&"fire"));

        let (h, w) = puzzle.dimensions();
        let total_visible: usize = puzzle.words().iter().map(|w| w.len()).sum();
        let total = total_visible + "fire".len();
        assert!(
            h * w >= total,
            "grid must fit at least {total} cells, got {}",
            h * w
        );

        let words: Vec<String> = puzzle.words().iter().map(|&s| s.to_string()).collect();
        assert!(solve(&puzzle.grid, &puzzle.placements, &words, "fire").is_ok());
    }

    #[test]
    fn exact_cell_count() {
        let config = PuzzleConfig {
            word_dictionary: words(&["abc", "def", "ghi", "jkl"]),
            solution_dictionary: hidden("xy", "test"),
            word_count: 3,
            ..default_config()
        };
        let puzzle = config.generate().unwrap();
        let (h, w) = puzzle.dimensions();
        let cells_used: usize = puzzle.placements.iter().map(|p| p.word.len()).sum();
        let hidden_len = puzzle.hidden_word.word.len();
        assert_eq!(h * w - cells_used, hidden_len);
    }

    #[test]
    fn leftover_is_scrambled_hidden_word() {
        let config = PuzzleConfig {
            word_dictionary: words(&["abc", "def", "ghi"]),
            solution_dictionary: hidden("xy", "test"),
            word_count: 2,
            ..default_config()
        };
        let puzzle = config.generate().unwrap();
        let (h, w) = puzzle.dimensions();
        let mut used = vec![vec![false; w]; h];
        for p in &puzzle.placements {
            let (dr, dc) = p.direction.delta();
            for (r, c) in positions(p.row, p.col, dr, dc, p.word.chars().count()) {
                used[r][c] = true;
            }
        }
        let leftover: String = (0..h)
            .flat_map(|r| (0..w).map(move |c| (r, c)))
            .filter(|&(r, c)| !used[r][c])
            .map(|(r, c)| puzzle.grid[r][c])
            .collect();

        let hidden_upper = puzzle.hidden_word.word.to_uppercase();
        let mut lc: Vec<char> = leftover.chars().collect();
        let mut hc: Vec<char> = hidden_upper.chars().collect();
        lc.sort();
        hc.sort();
        assert_eq!(lc, hc);
    }

    #[test]
    fn grid_only_contains_allowed_letters() {
        let config = PuzzleConfig {
            word_dictionary: words(&["rust", "python", "java", "go", "c"]),
            solution_dictionary: hidden("fire", "element"),
            word_count: 3,
            ..default_config()
        };
        let puzzle = config.generate().unwrap();
        let hidden_upper = puzzle.hidden_word.word.to_uppercase();
        let mut allowed: HashSet<char> = hidden_upper.chars().collect();
        for w in puzzle.words() {
            allowed.extend(w.to_uppercase().chars());
        }
        for (r, row) in puzzle.grid.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                assert!(
                    allowed.contains(&ch),
                    "cell ({r},{c}) has '{ch}' not in any word"
                );
            }
        }
    }

    #[test]
    fn orientation() {
        let dict = || words(&["abc", "def", "ghi", "jkl", "mno", "pqr"]);

        let square = PuzzleConfig {
            word_dictionary: dict(),
            solution_dictionary: hidden("test", "exam"),
            word_count: 4,
            orientation: Orientation::Square,
            ..default_config()
        }
        .generate()
        .unwrap();
        let (h, w) = square.dimensions();
        assert!(h.abs_diff(w) <= 2, "square: {w}x{h}");

        let landscape = PuzzleConfig {
            word_dictionary: dict(),
            solution_dictionary: hidden("xy", "test"),
            word_count: 4,
            orientation: Orientation::Landscape,
            ..default_config()
        }
        .generate()
        .unwrap();
        let (h, w) = landscape.dimensions();
        assert!(w >= h, "landscape: {w}x{h}");

        let portrait = PuzzleConfig {
            word_dictionary: dict(),
            solution_dictionary: hidden("xy", "test"),
            word_count: 4,
            orientation: Orientation::Portrait,
            ..default_config()
        }
        .generate()
        .unwrap();
        let (h, w) = portrait.dimensions();
        assert!(h >= w, "portrait: {w}x{h}");
    }

    #[test]
    fn direction_config() {
        let dict = || words(&["abc", "def", "ghi", "jkl", "mno", "pqr"]);

        let fwd = PuzzleConfig {
            word_dictionary: dict(),
            solution_dictionary: hidden("xy", "test"),
            word_count: 3,
            directions: DirectionConfig {
                horizontally: true,
                vertically: true,
                diagonally: true,
                backward: false,
            },
            ..default_config()
        }
        .generate()
        .unwrap();
        for p in &fwd.placements {
            match p.direction {
                Direction::E | Direction::S | Direction::SE | Direction::SW => {}
                _ => panic!("backward direction {:?}", p.direction),
            }
        }

        let horiz = PuzzleConfig {
            word_dictionary: dict(),
            solution_dictionary: hidden("xy", "test"),
            word_count: 3,
            directions: DirectionConfig {
                horizontally: true,
                vertically: false,
                diagonally: false,
                backward: false,
            },
            ..default_config()
        }
        .generate()
        .unwrap();
        for p in &horiz.placements {
            assert_eq!(p.direction, Direction::E);
        }
    }

    #[test]
    fn duplicate_is_filtered() {
        let conf = PuzzleConfig {
            word_dictionary: words(&["rust", "python", "java"]),
            solution_dictionary: hidden("rust", "language"),
            word_count: 2,
            ..default_config()
        };
        let puzzle = conf.generate().unwrap();
        assert_eq!(puzzle.placements.len(), 2);
        assert!(!puzzle.words().contains(&"rust"));

        let conf = PuzzleConfig {
            word_dictionary: words(&["aba", "aba", "aba"]),
            solution_dictionary: hidden("zz", "test"),
            word_count: 1,
            ..default_config()
        };
        let puzzle = conf.generate().unwrap();
        assert_eq!(puzzle.placements.len(), 1);
    }

    #[test]
    fn rejects_no_solution_words() {
        let config = PuzzleConfig {
            word_dictionary: words(&["hello"]),
            solution_dictionary: vec![],
            word_count: 1,
            ..default_config()
        };
        assert!(matches!(
            config.generate(),
            Err(PuzzleError::NoSolutionWords)
        ));
    }

    #[test]
    fn rejects_empty_word_dictionary() {
        let config = PuzzleConfig {
            word_dictionary: vec![],
            solution_dictionary: hidden("secret", "hidden"),
            word_count: 1,
            ..default_config()
        };
        assert!(matches!(
            config.generate(),
            Err(PuzzleError::EmptyWordDictionary)
        ));
    }

    #[test]
    fn rejects_invalid_word_count() {
        let config = PuzzleConfig {
            word_dictionary: words(&["hello"]),
            solution_dictionary: hidden("secret", "hidden"),
            word_count: 0,
            ..default_config()
        };
        assert!(matches!(
            config.generate(),
            Err(PuzzleError::InvalidWordCount)
        ));
    }

    #[test]
    fn rejects_no_directions() {
        let config = PuzzleConfig {
            word_dictionary: words(&["hello"]),
            solution_dictionary: hidden("secret", "hidden"),
            word_count: 1,
            directions: DirectionConfig {
                horizontally: false,
                vertically: false,
                diagonally: false,
                backward: false,
            },
            ..default_config()
        };
        assert!(matches!(config.generate(), Err(PuzzleError::NoDirections)));
    }

    #[test]
    fn rejects_empty_word() {
        let config = PuzzleConfig {
            word_dictionary: words(&["hello"]),
            solution_dictionary: hidden("", "empty"),
            word_count: 1,
            ..default_config()
        };
        assert!(matches!(
            config.generate(),
            Err(PuzzleError::EmptyWord { .. })
        ));

        let config = PuzzleConfig {
            word_dictionary: words(&["hello", ""]),
            solution_dictionary: hidden("secret", "hidden"),
            word_count: 1,
            ..default_config()
        };
        assert!(matches!(
            config.generate(),
            Err(PuzzleError::EmptyWord { .. })
        ));
    }
}
