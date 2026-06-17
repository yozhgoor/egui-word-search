# egui-word-search

[![crates.io](https://img.shields.io/crates/v/egui-word-search.svg)](https://crates.io/crates/egui-word-search)
[![docs.rs](https://docs.rs/egui-word-search/badge.svg)](https://docs.rs/egui-word-search)
[![deps.rs](https://deps.rs/repo/github/yozhgoor/egui-word-search/status.svg)](https://deps.rs/repo/github/yozhgoor/egui-word-search)
[![CI](https://github.com/yozhgoor/egui-word-search/actions/workflows/ci.yml/badge.svg)](https://github.com/yozhgoor/egui-word-search/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/egui-word-search.svg)](https://github.com/yozhgoor/egui-word-search#license)

A self-contained Word Search puzzle generator for [egui](https://github.com/emilk/egui) apps.

The generator takes a dictionary of words, a pool of hidden-word candidates with hints, and produces
a grid where every cell belongs to either a visible word or the hidden word, no random filler
letters are ever used. After the player finds all visible words, the remaining cells contain exactly
the letters of the hidden word, scrambled, with no repetition. Words can overlap when their letters
match at the crossing point. Grid orientation (square, landscape, portrait) and allowed directions
(horizontal, vertical, diagonal, backward) are all configurable.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
