//! Acceptance tests, one module per user story.
//!
//! These tests are written from the acceptance criteria before the implementation.
//! They are protected: never modify, delete or disable them.
//!
//! This file is the crate root of the test target, so `mod` declarations would be
//! looked up next to it in `tests/`. The `#[path]` attributes point them to
//! `tests/acceptance/` instead.

#[path = "acceptance/story_5.rs"]
mod story_5;

#[path = "acceptance/story_16.rs"]
mod story_16;

#[path = "acceptance/story_25.rs"]
mod story_25;

#[path = "acceptance/story_39.rs"]
mod story_39;

#[path = "acceptance/story_52.rs"]
mod story_52;

#[path = "acceptance/story_61.rs"]
mod story_61;

#[path = "acceptance/story_62.rs"]
mod story_62;

#[path = "acceptance/story_79.rs"]
mod story_79;

#[path = "acceptance/story_80.rs"]
mod story_80;

#[path = "acceptance/story_87.rs"]
mod story_87;

#[path = "acceptance/story_100.rs"]
mod story_100;

#[path = "acceptance/story_113.rs"]
mod story_113;

#[path = "acceptance/story_114.rs"]
mod story_114;

#[path = "acceptance/story_167.rs"]
mod story_167;

#[path = "acceptance/story_184.rs"]
mod story_184;

#[path = "acceptance/story_185.rs"]
mod story_185;

#[path = "acceptance/story_186.rs"]
mod story_186;

#[path = "acceptance/story_188.rs"]
mod story_188;

#[path = "acceptance/story_248.rs"]
mod story_248;

#[path = "acceptance/story_189.rs"]
mod story_189;
