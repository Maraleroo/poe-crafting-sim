//! Acceptance tests for AB#113 "Beschreibung und Fehlermeldung von Currencies anzeigen".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#113
//! and ADR 0012: the fields `description` and `messages` of `CurrencyDefinition`,
//! `CraftError::code()`, `Currency::description()` and `Currency::failure_message()`, together
//! with the unchanged interface of AB#62 (`currencies()`, `Currency`, `CraftError`,
//! `can_apply_orb_of_transmutation`, `apply_orb_of_transmutation`, `explicit_mods()`,
//! `base_item`, `Item::new`, `SeededRng`). Following ADR 0004, web criteria are checked through
//! the file contents under `web/` (without the build output in `web/dist` and `web/target`).
//!
//! The PO's decisions on the story: descriptions and failure messages are game texts and
//! therefore game data in `data/currencies.json`, not UI texts. A currency that cannot be
//! applied shows its message on hover and on click. For AC4, "no longer used" means the
//! delivered website and everything it is built from: the file contents of `web/`,
//! `crafting-core/src` and `data/`. Historical mentions in ADRs and in test comments that
//! quote old criteria stay.
//!
//! Red state before the implementation: the module does not compile, because the new
//! interface (`description`, `messages`, `code`, `failure_message`) does not exist yet. AC5 is
//! a text check of the PO's rules, which were committed to `poe-crafting-rules` before these
//! tests (841b808); it passes as soon as the module compiles.
//!
//! Adapted for AB#248 (PO-confirmed): criterion 3 of AB#248 replaces AC4 and rule 4 in AC5.
//! Without a valid mod, the orb takes effect and the item becomes magic with no mods; the orb
//! never shows "Failed to apply item: Could not generate Mod.", and `CraftError::NoValidMod`
//! is gone.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC1 and AC2: the tooltip on hover and how the inapplicable orb looks.
//! - AC3: the message after a click, also in a narrow window (mobile).

use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_transmutation, base_item, can_apply_orb_of_transmutation, currencies,
    explicit_mods, BaseItem, CraftError, Currency, CurrencyDefinition, ExplicitModDefinition, Item,
    Rarity, SeededRng,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story
// ---------------------------------------------------------------------------------------

/// The Iron Ring from AB#52, the item shown on the page.
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";

const ORB_ID: &str = "orb_of_transmutation";

/// The game texts from the criteria, byte for byte from AB#113.
const ORB_DESCRIPTION: &str = "Upgrades a normal item to a magic item";
const NOT_NORMAL_MESSAGE: &str = "Failed to apply item: Target is not Normal Rarity.";
const NO_VALID_MOD_MESSAGE: &str = "Failed to apply item: Could not generate Mod.";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_NORMAL_CODE: &str = "item_not_normal";

/// The project's own text that the game message replaces (AC4), and its translation key
/// (removed according to the tech lead's plan).
const OLD_NO_VALID_MOD_TEXT: &str =
    "No mod can be added: this item has no valid mods to roll from.";
/// Parts of the old text, so that a copy split over lines is found as well.
const OLD_NO_VALID_MOD_PARTS: [&str; 2] = ["No mod can be added", "no valid mods to roll from"];
const OLD_NO_VALID_MOD_KEY: &str = "craft.error.no_valid_mod";

/// The PO's rule lines (AC5), byte for byte from AB#113.
const APPLIES_TO_LINE: &str = "- Applies to: normal items.";
const NOT_APPLICABLE_LINE: &str = "- Not applicable when: the item is not normal. The reason \
     shown is the game's message \"Failed to apply item: Target is not Normal Rarity.\"";

const CURRENCIES_JSON: &str = "data/currencies.json";
const ORB_RULE_FILE: &str = ".claude/skills/poe-crafting-rules/currencies/orb-of-transmutation.md";

/// Ways to set the HTML attribute `disabled` on the orb button from Rust, which would stop
/// the click of AC3 from arriving. `"aria-disabled"` does not contain `"disabled"` with the
/// opening quote, so it is not matched.
const DISABLED_ATTRIBUTE: [&str; 2] = ["set_disabled", "\"disabled\""];

/// Fixed seed, so every run is identical.
const SEED: u64 = 113;
/// Items for checks over many results.
const ITEMS: u64 = 500;

// ---------------------------------------------------------------------------------------
// Helpers: files
// ---------------------------------------------------------------------------------------

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_path(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    assert!(path.is_file(), "{relative} must exist");
    read_path(&path)
}

/// All files below `dir`, skipping the given direct subdirectories (build output).
fn files_below(dir: &Path, skipped: &[&str]) -> Vec<PathBuf> {
    fn collect(root: &Path, dir: &Path, skipped: &[&str], out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", dir.display()))
                .path();
            let relative = path.strip_prefix(root).expect("path lies under its root");
            if skipped.iter().any(|skip| relative == Path::new(skip)) {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, skipped, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(dir, dir, skipped, &mut files);
    files
}

/// All files under `web/`, except build output in `web/dist` and `web/target`.
fn web_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("web"), &["dist", "target"]);
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "rs")),
        "web/ must contain Rust source files; the scan found none"
    );
    files
}

/// All files under `web/src/`.
fn web_src_files() -> Vec<PathBuf> {
    let src = repo_path("web").join("src");
    let files: Vec<PathBuf> = web_files()
        .into_iter()
        .filter(|path| path.starts_with(&src))
        .collect();
    assert!(!files.is_empty(), "web/src/ must contain files");
    files
}

/// All files under `crafting-core/src/`.
fn core_src_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("crafting-core").join("src"), &[]);
    assert!(!files.is_empty(), "crafting-core/src/ must contain files");
    files
}

/// All files under `data/`.
fn data_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("data"), &[]);
    assert!(!files.is_empty(), "data/ must contain files");
    files
}

/// Files whose text contains `needle`.
fn files_containing(files: &[PathBuf], needle: &str) -> Vec<String> {
    files
        .iter()
        .filter(|path| read_path(path).contains(needle))
        .map(|path| path.display().to_string())
        .collect()
}

/// "<file> contains <needle>" for every needle found in one of the files.
fn findings(files: &[PathBuf], needles: &[&str]) -> Vec<String> {
    needles
        .iter()
        .flat_map(|needle| {
            files_containing(files, needle)
                .into_iter()
                .map(move |file| format!("{file} contains {needle:?}"))
        })
        .collect()
}

/// Asserts that some file under `web/src/` contains each of the needles.
fn assert_web_src_uses(needles: &[&str], why: &str) {
    let src = web_src_files();
    for needle in needles {
        assert!(
            !files_containing(&src, needle).is_empty(),
            "a file under web/src/ must use `{needle}`: {why}"
        );
    }
}

/// Asserts that a game text lives only in the game data: not in the code of `crafting-core`
/// and not in any file under `web/` (ADR 0012, as for currency names in ADR 0011).
fn assert_only_in_game_data(text: &str) {
    let in_code = [
        files_containing(&core_src_files(), text),
        files_containing(&web_files(), text),
    ]
    .concat();
    assert!(
        in_code.is_empty(),
        "{text:?} must only be in {CURRENCIES_JSON}, also found in: {in_code:#?}"
    );
}

// ---------------------------------------------------------------------------------------
// Helpers: game data
// ---------------------------------------------------------------------------------------

/// The entry of the Orb of Transmutation in `data/currencies.json`.
fn orb_entry_in_file() -> Value {
    let content = read_text(CURRENCIES_JSON);
    let data: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    data.get("currencies")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must have an array \"currencies\""))
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(ORB_ID))
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {ORB_ID:?}"))
        .clone()
}

/// The text of the orb's entry in `data/currencies.json` at the given path of keys.
fn text_in_file(path: &[&str]) -> String {
    let entry = orb_entry_in_file();
    let mut value = &entry;
    for key in path {
        value = value.get(key).unwrap_or_else(|| {
            panic!("{CURRENCIES_JSON}: the entry {ORB_ID:?} must have {path:?}, found {entry}")
        });
    }
    value
        .as_str()
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {ORB_ID:?} {path:?} must be a string"))
        .to_owned()
}

/// The entry of the Orb of Transmutation in `currencies()`.
fn orb_definition() -> CurrencyDefinition {
    currencies()
        .into_iter()
        .find(|entry| entry.id == ORB_ID)
        .unwrap_or_else(|| panic!("currencies() must contain {ORB_ID:?}"))
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn iron_ring() -> BaseItem {
    base_item(IRON_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer the base item {IRON_RING_ID:?}"))
}

/// A magic Iron Ring, made with the orb from the mod pool of crafting-core.
fn magic_ring(base: &BaseItem, pool: &[ExplicitModDefinition], rng: &mut SeededRng) -> Item {
    let mut item = Item::new(base, rng);
    apply_orb_of_transmutation(&mut item, pool, rng).expect("the orb must apply to a normal ring");
    assert_eq!(item.rarity, Rarity::Magic);
    item
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Ist der Orb of Transmutation anwendbar (normales Item), zeigt das Überfahren mit der
// Maus genau den Text "Upgrades a normal item to a magic item"."
// (The tooltip itself is checked by the PO in the local preview. Here: the orb is applicable
// to the normal item, its description from crafting-core and the game data, and that the
// page shows it.)
#[test]
fn ac1_applicable_orb_shows_the_game_description_on_hover() {
    // Given the normal Iron Ring
    let base = iron_ring();
    let mut rng = SeededRng::new(SEED);
    let item = Item::new(&base, &mut rng);
    assert_eq!(item.rarity, Rarity::Normal);

    // When the page asks whether the orb applies and for the orb's description
    let applicable = can_apply_orb_of_transmutation(&item);
    let description = Currency::OrbOfTransmutation.description();

    // Then the orb is applicable
    assert_eq!(applicable, Ok(()), "the orb applies to a normal item");
    // And its description is exactly the game text
    assert_eq!(description, ORB_DESCRIPTION);
    // And the text is game data: in currencies() and in data/currencies.json
    assert_eq!(orb_definition().description, ORB_DESCRIPTION);
    assert_eq!(text_in_file(&["description"]), ORB_DESCRIPTION);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(ORB_DESCRIPTION);
    // And the page takes the description from crafting-core
    assert_web_src_uses(
        &["description()"],
        "the orb's tooltip on a normal item is its description from crafting-core",
    );
}

// AC2: „Ist er nicht anwendbar (magisches Item), wird er als nicht anwendbar dargestellt, und
// das Überfahren mit der Maus zeigt genau den Text "Failed to apply item: Target is not
// Normal Rarity."."
// (How the orb looks and the tooltip are checked by the PO in the local preview. Here: the
// reason code, the message for it from crafting-core and the game data, and that the page
// marks the orb as not applicable and shows the message.)
#[test]
fn ac2_inapplicable_orb_is_marked_and_shows_the_game_message_on_hover() {
    // Given magic Iron Rings
    let base = iron_ring();
    let pool = explicit_mods();
    let mut rng = SeededRng::new(SEED);

    for _ in 0..ITEMS {
        let item = magic_ring(&base, &pool, &mut rng);

        // When the page asks whether the orb applies
        let result = can_apply_orb_of_transmutation(&item);

        // Then it does not, because the item is not normal
        assert_eq!(result, Err(CraftError::ItemNotNormal));
        // And the message for this reason is exactly the game text
        let error = result.expect_err("the orb does not apply to a magic item");
        assert_eq!(
            Currency::OrbOfTransmutation.failure_message(error),
            NOT_NORMAL_MESSAGE
        );
    }
    // And the reason code links the error to the game data
    assert_eq!(CraftError::ItemNotNormal.code(), NOT_NORMAL_CODE);
    let definition = orb_definition();
    assert_eq!(
        definition.messages.get(NOT_NORMAL_CODE).map(String::as_str),
        Some(NOT_NORMAL_MESSAGE),
        "currencies(): message of {ORB_ID:?} for {NOT_NORMAL_CODE:?}"
    );
    assert_eq!(
        text_in_file(&["messages", NOT_NORMAL_CODE]),
        NOT_NORMAL_MESSAGE
    );
    // And the text is not written into the code of crafting-core or web
    assert_only_in_game_data(NOT_NORMAL_MESSAGE);
    // And the page shows the message from crafting-core and marks the orb as not applicable
    assert_web_src_uses(
        &[
            "can_apply_orb_of_transmutation",
            "failure_message",
            "aria-disabled",
        ],
        "an inapplicable orb is marked as such and its tooltip is the failure message",
    );
}

// AC3: „Ein Klick auf den nicht anwendbaren Orb zeigt dieselbe Meldung. Das Item bleibt
// unverändert."
// (The message area after the click is checked by the PO in the local preview. Here: the
// click's result in crafting-core, that its message is the same as on hover, and that the
// button can still be clicked.)
#[test]
fn ac3_click_on_inapplicable_orb_shows_the_same_message_and_keeps_the_item() {
    // Given magic Iron Rings
    let base = iron_ring();
    let pool = explicit_mods();
    let mut rng = SeededRng::new(SEED);

    for _ in 0..ITEMS {
        let mut item = magic_ring(&base, &pool, &mut rng);
        let before = item.clone();
        let hover_error = can_apply_orb_of_transmutation(&item)
            .expect_err("the orb does not apply to a magic item");

        // When the orb is clicked, so the page applies it
        let result = apply_orb_of_transmutation(&mut item, &pool, &mut rng);

        // Then it fails with the same reason as on hover
        assert_eq!(result, Err(hover_error));
        // And its message is the same game text as on hover
        let click_error = result.expect_err("applying the orb to a magic item fails");
        assert_eq!(
            Currency::OrbOfTransmutation.failure_message(click_error),
            Currency::OrbOfTransmutation.failure_message(hover_error)
        );
        assert_eq!(
            Currency::OrbOfTransmutation.failure_message(click_error),
            NOT_NORMAL_MESSAGE
        );
        // And the item is unchanged
        assert_eq!(item, before);
    }
    // And the page shows the failure message for a click
    assert_web_src_uses(
        &["apply_orb_of_transmutation", "failure_message"],
        "a click on the orb shows the failure message from crafting-core",
    );
    // And the orb button does not get the HTML attribute `disabled`, which would swallow the
    // click
    let disabled = findings(&web_src_files(), &DISABLED_ATTRIBUTE);
    assert!(
        disabled.is_empty(),
        "an inapplicable orb must stay clickable, so web/src/ must not set the HTML attribute \
         `disabled` (use aria-disabled instead): {disabled:#?}"
    );
}

// AC4: „Gibt es keinen gültigen Mod, zeigt das Anwenden des Orbs genau die Meldung "Failed to
// apply item: Could not generate Mod.". Der Text "No mod can be added: this item has no
// valid mods to roll from." wird nicht mehr verwendet."
// (PO's answer: "no longer used" means the delivered website and the file contents of
// `web/`, `crafting-core/src` and `data/`.)
// (Adapted for AB#248, PO-confirmed: criterion 3 of AB#248 replaces the first sentence. Without
// a valid mod, the orb takes effect, the ring becomes magic with no mods, and the orb never
// shows "Failed to apply item: Could not generate Mod."; its message is removed from the game
// data. The check that the old text is gone stays. Renamed from
// `ac4_no_valid_mod_shows_the_game_message_and_the_old_text_is_gone`.)
#[test]
fn ac4_without_a_valid_mod_the_orb_takes_effect_and_the_old_text_is_gone() {
    // Given normal Iron Rings and a pool without any valid mod
    let base = iron_ring();
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let mut rng = SeededRng::new(SEED);

    for _ in 0..ITEMS {
        let mut item = Item::new(&base, &mut rng);
        let before = item.clone();

        // When the orb is applied
        let result = apply_orb_of_transmutation(&mut item, &empty, &mut rng);

        // Then it takes effect: the ring becomes magic with no mods at all
        assert_eq!(result, Ok(()));
        assert_eq!(item.rarity, Rarity::Magic);
        assert!(item.prefixes.is_empty() && item.suffixes.is_empty());
        assert_eq!(item.base, before.base);
        assert_eq!(item.implicits, before.implicits);
    }
    // And the orb has no message "Could not generate Mod.", in crafting-core and in the game data
    let definition = orb_definition();
    assert!(
        definition
            .messages
            .values()
            .all(|message| message.as_str() != NO_VALID_MOD_MESSAGE),
        "currencies(): {ORB_ID:?} must have no message {NO_VALID_MOD_MESSAGE:?}, found {:?}",
        definition.messages
    );
    let entry = orb_entry_in_file();
    let file_messages = entry
        .get("messages")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {ORB_ID:?} must have \"messages\""));
    assert!(
        file_messages
            .values()
            .all(|message| message.as_str() != Some(NO_VALID_MOD_MESSAGE)),
        "{CURRENCIES_JSON}: {ORB_ID:?} must have no message {NO_VALID_MOD_MESSAGE:?}, found \
         {file_messages:?}"
    );
    // And the text is not written into the code of crafting-core or web
    assert_only_in_game_data(NO_VALID_MOD_MESSAGE);

    // And the old text is no longer used: not on the website and not in what it is built from
    let built_from = [web_files(), core_src_files(), data_files()].concat();
    let old_text = findings(&built_from, &OLD_NO_VALID_MOD_PARTS);
    assert!(
        old_text.is_empty(),
        "{OLD_NO_VALID_MOD_TEXT:?} must no longer be used in web/, crafting-core/src or data/: \
         {old_text:#?}"
    );
    // And neither is its translation key
    let old_key = findings(&web_files(), &[OLD_NO_VALID_MOD_KEY]);
    assert!(
        old_key.is_empty(),
        "the translation key {OLD_NO_VALID_MOD_KEY:?} must be removed from web/: {old_key:#?}"
    );
}

// AC5: „`poe-crafting-rules`, Datei `currencies/orb-of-transmutation.md`:
//    - Nach der Zeile "- Applies to: normal items." steht genau diese Zeile:
//      - Not applicable when: the item is not normal. The reason shown is the game's message
//        "Failed to apply item: Target is not Normal Rarity."
//    - Regel 4 lautet genau:
//      4. If there is no valid mod at all, applying the orb fails with the game's message
//         "Failed to apply item: Could not generate Mod." and the item stays unchanged."
// (Adapted for AB#248, PO-confirmed: criterion 3 of AB#248 replaces rules 1 to 4 of the file,
// so rule 4 is no longer checked here. Its wording is checked at the acceptance of AB#248. The
// "Not applicable when" line stays.)
#[test]
fn ac5_orb_rule_file_contains_the_po_lines_verbatim() {
    // Given the PO's rule file for the Orb of Transmutation
    let rules = read_text(ORB_RULE_FILE);
    let lines: Vec<&str> = rules.lines().map(str::trim_end).collect();

    // When looking for the line "- Applies to: normal items."
    let applies_to = lines
        .iter()
        .position(|line| *line == APPLIES_TO_LINE)
        .unwrap_or_else(|| panic!("{ORB_RULE_FILE} must contain the line {APPLIES_TO_LINE:?}"));

    // Then exactly the PO's "Not applicable when" line follows it directly
    assert_eq!(
        lines.get(applies_to + 1).copied(),
        Some(NOT_APPLICABLE_LINE),
        "in {ORB_RULE_FILE}, the line after {APPLIES_TO_LINE:?} must be the PO's line"
    );
}
