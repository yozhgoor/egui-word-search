// The #[run_example] macro generates:
//   - wasm32: A #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example]
fn run() {
    use eframe;
    use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Vec2};
    use egui_word_search::WordPlacement;
    use egui_word_search::{DirectionConfig, HiddenWord, Orientation, Puzzle, PuzzleConfig};
    use std::collections::HashSet;
    use wasm_bindgen_futures;
    use web_sys;
    use xtask_wasm::wasm_bindgen::JsCast;

    const WORDS: &[&str] = &[
        "RUST", "CRAB", "CODE", "MATCH", "CARGO", "STACK", "HEAP", "CLONE", "MACRO", "CRATE",
        "TRAIT", "ENUM", "IMPL", "DEBUG", "LOGIC", "ERROR", "TOKEN", "SLICE", "GUARD", "LOOP",
        "MAP", "HASH", "TREE", "GRAPH", "QUEUE", "ARRAY", "BYTE", "TEXT", "NODE", "SCALE",
    ];

    const HIDDEN_WORDS: &[(&str, &str)] = &[
        ("PATTERN", "A recurring design structure"),
        ("COMPILE", "Turn source code into machine code"),
        ("SYNTAX", "Rules for writing valid code"),
        ("THREAD", "Lightweight unit of execution"),
        ("HIDDEN", "Concealed from plain sight"),
        ("SEARCH", "What you are doing right now"),
        ("SOLVER", "One who finds solutions"),
        ("CIPHER", "A secret method of writing"),
        ("LINKER", "Combines compiled object files"),
        ("BINARY", "Zeros and ones"),
    ];

    struct WordSearchApp {
        game: Game,
        difficulty: Difficulty,
        dark_mode: bool,
        guess_input: String,
        guess_result: Option<bool>,
    }

    impl Default for WordSearchApp {
        fn default() -> Self {
            let mut app = Self {
                game: Game::new(),
                difficulty: Difficulty::Easy,
                dark_mode: true,
                guess_input: String::new(),
                guess_result: None,
            };
            let config = build_puzzle_config(app.difficulty);
            app.game.new_puzzle(&config);
            app
        }
    }

    impl eframe::App for WordSearchApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            ui.ctx().set_visuals(if self.dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            egui::Panel::top("top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button("Settings", |ui| {
                        ui.label("Difficulty:");
                        for diff in Difficulty::all() {
                            let before = self.difficulty;
                            let resp = ui.radio_value(&mut self.difficulty, diff, diff.name());
                            if resp.clicked() && before != diff {
                                let config = build_puzzle_config(diff);
                                self.game.new_puzzle(&config);
                                self.guess_input.clear();
                                self.guess_result = None;
                            }
                        }
                        ui.separator();
                        if ui.checkbox(&mut self.dark_mode, "Dark mode").clicked() {
                            ui.ctx().set_visuals(if self.dark_mode {
                                egui::Visuals::dark()
                            } else {
                                egui::Visuals::light()
                            });
                        }
                        ui.separator();
                        if ui.button("New Game").clicked() {
                            let config = build_puzzle_config(self.difficulty);
                            self.game.new_puzzle(&config);
                            self.guess_input.clear();
                            self.guess_result = None;
                        }
                    });

                    ui.separator();

                    if let Some(puzzle) = &self.game.puzzle {
                        ui.label(
                            egui::RichText::new(format!("Hint: {}", puzzle.hidden_word.hint))
                                .color(Color32::LIGHT_BLUE),
                        );

                        let resp = ui.add_sized(
                            egui::vec2(120.0, ui.spacing().interact_size.y),
                            egui::TextEdit::singleline(&mut self.guess_input)
                                .hint_text("guess hidden word"),
                        );
                        let guess_btn = ui.add_enabled(
                            !self.guess_input.trim().is_empty(),
                            egui::Button::new("Guess"),
                        );

                        let should_guess = guess_btn.clicked()
                            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

                        if should_guess && !self.guess_input.trim().is_empty() {
                            let guess = self.guess_input.trim().to_uppercase();
                            let correct = guess == puzzle.hidden_word.word.to_uppercase();
                            self.guess_result = Some(correct);
                        }

                        match self.guess_result {
                            Some(true) => {
                                ui.colored_label(Color32::GREEN, "Correct!");
                            }
                            Some(false) => {
                                ui.colored_label(Color32::RED, "Wrong!");
                            }
                            None => {}
                        }
                    }
                });
            });

            if self.game.all_visible_found() && self.guess_result == Some(true) {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height().max(0.0) * 0.3);
                    ui.heading("Congratulations!");
                    ui.label("You found all the words and cracked the hidden word!");
                    ui.add_space(10.0);
                    if ui.button("New Game").clicked() {
                        let config = build_puzzle_config(self.difficulty);
                        self.game.new_puzzle(&config);
                        self.guess_input.clear();
                        self.guess_result = None;
                    }
                });
            } else {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    let Some(puzzle) = &self.game.puzzle else {
                        return;
                    };

                    let rows = puzzle.grid.len();
                    let cols = puzzle.grid[0].len();

                    let cell_size = (ui.available_size().x / cols as f32)
                        .min(ui.available_size().y / rows as f32)
                        .max(20.0)
                        .min(48.0);

                    let total = Vec2::new(cols as f32 * cell_size, rows as f32 * cell_size);
                    let (response, painter) = ui.allocate_painter(total, Sense::click_and_drag());
                    let origin = response.rect.min;

                    let grid = puzzle.grid.clone();
                    let placements = puzzle.placements.clone();
                    let found_set = compute_found_set(puzzle, &self.game.found_words);
                    let selection_set = cells_on_line(self.game.drag_start, self.game.drag_end);
                    let visuals = ui.visuals();

                    for (r, row_cells) in grid.iter().enumerate() {
                        for (c, &letter) in row_cells.iter().enumerate() {
                            let cell_rect = Rect::from_min_size(
                                origin + Vec2::new(c as f32, r as f32) * cell_size,
                                Vec2::splat(cell_size),
                            );

                            let inner = cell_rect.shrink(2.0);
                            let rounding = CornerRadius::same(4);

                            let bg = if found_set.contains(&(r, c)) {
                                Color32::from_rgb(46, 125, 50)
                            } else if selection_set.contains(&(r, c)) {
                                visuals.selection.bg_fill
                            } else {
                                visuals.widgets.inactive.bg_fill
                            };

                            painter.rect_filled(inner, rounding, bg);

                            let text_color =
                                if found_set.contains(&(r, c)) || selection_set.contains(&(r, c)) {
                                    Color32::WHITE
                                } else {
                                    visuals.text_color()
                                };

                            painter.text(
                                cell_rect.center(),
                                Align2::CENTER_CENTER,
                                letter.to_string(),
                                FontId::proportional(cell_size * 0.55),
                                text_color,
                            );
                        }
                    }

                    if response.is_pointer_button_down_on() {
                        if let Some(pos) = ui.ctx().pointer_interact_pos() {
                            let local = pos - origin;
                            let cx = (local.x / cell_size).floor() as usize;
                            let cy = (local.y / cell_size).floor() as usize;
                            if cx < cols && cy < rows {
                                if self.game.drag_start.is_none() {
                                    self.game.drag_start = Some((cy, cx));
                                }
                                self.game.drag_end = Some((cy, cx));
                            }
                        }
                    } else if self.game.drag_start.is_some() {
                        let (start, end) = (self.game.drag_start, self.game.drag_end);
                        if let (Some(start), Some(end)) = (start, end) {
                            let line = cells_on_line(Some(start), Some(end));
                            if !line.is_empty() {
                                let mut word = String::new();
                                for &(r, c) in &line {
                                    word.push(grid[r][c]);
                                }
                                let word_upper = word.to_uppercase();
                                let rev_upper: String =
                                    word.chars().rev().collect::<String>().to_uppercase();
                                for placement in &placements {
                                    let pw = placement.word.to_uppercase();
                                    if word_upper == pw || rev_upper == pw {
                                        self.game.found_words.insert(pw);
                                        break;
                                    }
                                }
                            }
                        }
                        self.game.drag_start = None;
                        self.game.drag_end = None;
                    }
                });
            }

            egui::Panel::bottom("bottom").show_inside(ui, |ui| {
                if let Some(puzzle) = &self.game.puzzle {
                    ui.label("Words to find:");
                    ui.horizontal_wrapped(|ui| {
                        let mut placements: Vec<&WordPlacement> =
                            puzzle.placements.iter().collect();
                        placements.sort_by(|a, b| a.word.cmp(&b.word));
                        for placement in &placements {
                            let found = self
                                .game
                                .found_words
                                .contains(&placement.word.to_uppercase());
                            let text = if found {
                                format!("✓ {}", placement.word)
                            } else {
                                placement.word.clone()
                            };
                            let mut rt = egui::RichText::new(text);
                            if found {
                                rt = rt.strikethrough().color(Color32::GREEN);
                            } else {
                                rt = rt.color(ui.visuals().text_color());
                            }
                            ui.add(egui::Label::new(rt));
                            ui.add_space(8.0);
                        }
                    });
                }
            });
        }
    }

    struct Game {
        puzzle: Option<Puzzle>,
        found_words: HashSet<String>,
        drag_start: Option<(usize, usize)>,
        drag_end: Option<(usize, usize)>,
    }

    impl Game {
        fn new() -> Self {
            Self {
                puzzle: None,
                found_words: HashSet::new(),
                drag_start: None,
                drag_end: None,
            }
        }

        fn new_puzzle(&mut self, config: &PuzzleConfig) {
            self.puzzle = Some(config.generate().expect("puzzle generation failed"));
            self.found_words.clear();
            self.drag_start = None;
            self.drag_end = None;
        }

        fn all_visible_found(&self) -> bool {
            let Some(puzzle) = &self.puzzle else {
                return false;
            };
            puzzle
                .placements
                .iter()
                .all(|p| self.found_words.contains(&p.word.to_uppercase()))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Difficulty {
        Easy,
        Medium,
        Hard,
    }

    impl Difficulty {
        fn all() -> [Difficulty; 3] {
            [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard]
        }

        fn name(&self) -> &'static str {
            match self {
                Difficulty::Easy => "Easy",
                Difficulty::Medium => "Medium",
                Difficulty::Hard => "Hard",
            }
        }

        fn word_count(&self) -> usize {
            match self {
                Difficulty::Easy => 6,
                Difficulty::Medium => 8,
                Difficulty::Hard => 10,
            }
        }

        fn direction_config(&self) -> DirectionConfig {
            match self {
                Difficulty::Easy => DirectionConfig {
                    horizontally: true,
                    vertically: true,
                    diagonally: false,
                    backward: false,
                },
                Difficulty::Medium => DirectionConfig {
                    horizontally: true,
                    vertically: true,
                    diagonally: true,
                    backward: false,
                },
                Difficulty::Hard => DirectionConfig {
                    horizontally: true,
                    vertically: true,
                    diagonally: true,
                    backward: true,
                },
            }
        }
    }

    fn build_puzzle_config(difficulty: Difficulty) -> PuzzleConfig {
        let solution_dictionary: Vec<HiddenWord> = HIDDEN_WORDS
            .iter()
            .map(|&(w, h)| HiddenWord {
                word: w.to_string(),
                hint: h.to_string(),
            })
            .collect();

        let word_dictionary: Vec<String> = WORDS.iter().map(|s| s.to_string()).collect();

        PuzzleConfig {
            word_dictionary,
            solution_dictionary,
            word_count: difficulty.word_count(),
            orientation: Orientation::Square,
            directions: difficulty.direction_config(),
            max_word_attempts: 200,
            max_grid_attempts: 20,
        }
    }

    fn compute_found_set(
        puzzle: &Puzzle,
        found_words: &HashSet<String>,
    ) -> HashSet<(usize, usize)> {
        let mut cells = HashSet::new();
        for placement in &puzzle.placements {
            if found_words.contains(&placement.word.to_uppercase()) {
                let (dr, dc) = placement.direction.delta();
                for i in 0..placement.word.len() {
                    let r = (placement.row as isize + i as isize * dr) as usize;
                    let c = (placement.col as isize + i as isize * dc) as usize;
                    cells.insert((r, c));
                }
            }
        }
        cells
    }

    fn cells_on_line(
        start: Option<(usize, usize)>,
        end: Option<(usize, usize)>,
    ) -> Vec<(usize, usize)> {
        let (Some(start), Some(end)) = (start, end) else {
            return Vec::new();
        };

        let dr = (end.0 as isize - start.0 as isize).signum();
        let dc = (end.1 as isize - start.1 as isize).signum();

        let is_valid = dr == 0 || dc == 0 || dr.abs() == dc.abs();
        if !is_valid {
            return Vec::new();
        }

        let steps = (end.0 as isize - start.0 as isize)
            .abs()
            .max((end.1 as isize - start.1 as isize).abs());
        let mut cells = Vec::new();
        for i in 0..=steps {
            let r = (start.0 as isize + i * dr) as usize;
            let c = (start.1 as isize + i * dc) as usize;
            cells.push((r, c));
        }
        cells
    }

    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let canvas = document
        .create_element("canvas")
        .expect("failed to create canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("not a HtmlCanvasElement");

    let style = canvas.style();
    style
        .set_property("position", "fixed")
        .expect("failed to set position property");
    style
        .set_property("top", "0")
        .expect("failed to set top property");
    style
        .set_property("left", "0")
        .expect("failed to set left property");
    style
        .set_property("width", "100%")
        .expect("failed to set width property");
    style
        .set_property("height", "100%")
        .expect("failed to set height property");

    let body = document.body().expect("no body");
    body.style()
        .set_property("margin", "0")
        .expect("failed to set margin property");
    body.append_child(&canvas).expect("failed to append canvas");

    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(WordSearchApp::default()))),
            )
            .await
            .expect("failed to start eframe");
    });
}
