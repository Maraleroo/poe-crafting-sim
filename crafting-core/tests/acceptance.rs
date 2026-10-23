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
