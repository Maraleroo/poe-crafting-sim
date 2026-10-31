//! Acceptance tests for AB#114 "Orb of Alteration auf den magischen Iron Ring anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#114
//! and ADR 0013: `valid_mods`, `CraftError::ItemNotMagic`, `can_apply_orb_of_alteration`,
//! `apply_orb_of_alteration` and `Currency::OrbOfAlteration` with its texts in
//! `data/currencies.json`, together with the unchanged interface of AB#62 and AB#113
//! (`explicit_mods`, `currencies`, `Currency`, `CraftError::code`, `apply_orb_of_transmutation`,
//! `base_item`, `Item::new`, `Mod::roll`, `SeededRng`). Following ADR 0004, web criteria are
//! checked through the file contents under `web/` (without the build output in `web/dist` and
//! `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`, `currencies/orb-of-alteration.md` and the section
//! "Item level". The Iron Ring's pool is `valid_mods(&explicit_mods(), 100)`, the item level
//! the UI uses by default (ADR 0013). Every statistical test uses a fixed seed and compares
//! observed shares with the shares that follow from the mod weights, with a tolerance of 5
//! standard deviations. The tests depend only on frequencies and invariants, never on the
//! order in which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because the new
//! interface does not exist yet. AC5 and AC6 also check the PO's rules, which were committed
//! to `poe-crafting-rules` before these tests (369228c); those parts pass as soon as the
//! module compiles.
//!
//! Adapted for AB#248 (PO-confirmed): criterion 5 of AB#248 replaces the fields `prefix` and
//! `suffix` of `Item` by the lists `prefixes` and `suffixes` (ADR 0021). The tests read and set
//! the lists; what they check is unchanged. Criteria 1 to 3 of AB#248 replace rule 4 in AC2 and
//! the PO's old wording in AC5. Criterion 4 of AB#248 replaces the check in AC6 that the page
//! builds the pool with `valid_mods`.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC1: the second button next to the Orb of Transmutation.
//! - AC3 and AC4: the tooltip on hover and the message after a click.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_transmutation, base_item, can_apply_orb_of_alteration,
    currencies, explicit_mods, valid_mods, AffixType, BaseItem, CraftError, Currency,
    CurrencyDefinition, ExplicitMod, ExplicitModDefinition, Item, Mod, ModDefinition, Rarity,
    SeededRng, StatDefinition,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story
// ---------------------------------------------------------------------------------------

/// The Iron Ring from AB#52, the item shown on the page.
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";

const ORB_ID: &str = "orb_of_alteration";
const TRANSMUTATION_ID: &str = "orb_of_transmutation";

/// The game texts from the criteria, byte for byte from AB#114.
const ORB_NAME: &str = "Orb of Alteration";
const ORB_DESCRIPTION: &str = "Reforges a magic item with new random modifiers";
const NOT_MAGIC_MESSAGE: &str = "Failed to apply item: Target is not Magic.";
const NO_VALID_MOD_MESSAGE: &str = "Failed to apply item: Could not generate Mod.";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_MAGIC_CODE: &str = "item_not_magic";

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// The lines of the PO's rule file for the Orb of Alteration (AC5) that AB#248 does not
/// replace: the first three lines of the file. Adapted for AB#248 (PO-confirmed): criterion 3
/// of AB#248 replaces the "Effect" line, rules 1 to 4 and the sentence on the empty pool in the
/// evidence. Their wording is checked at the acceptance of AB#248.
const ORB_RULE_LINES: [&str; 3] = [
    "**Orb of Alteration**",
    "- Applies to: magic items.",
    "- Not applicable when: the item is not magic. The reason shown is the game's message \
     \"Failed to apply item: Target is not Magic.\"",
];
const ORB_RULE_LINK: &str = "currencies/orb-of-alteration.md";
const CURRENCY_RULES_HEADING: &str = "Currency rules:";

/// The PO's lines for the section "Item level" (AC6), and the line they replace.
const ITEM_LEVEL_RULE: &str = "A mod can roll on an item only if the item level is at least \
     the mod's level. Example: a mod with level 60 needs an item level of 60 or higher. Default \
     in the UI: 100.";
const ITEM_LEVEL_EVIDENCE: &str = "- Evidence: the PO's knowledge of the game. Consistent with \
     the PO's measurement on 2026-10-21 (3000 Orbs of Transmutation and Alteration on a Bone \
     Ring of item level 83): mods with level 82 rolled 63 times, mods with level 84 or 85 \
     never.";
const ITEM_LEVEL_TODO: &str =
    "TODO (PO): how item level restricts which mods can roll. Default in the UI: 100.";

const CURRENCIES_JSON: &str = "data/currencies.json";
const CRAFTING_RULES: &str = ".claude/skills/poe-crafting-rules/SKILL.md";
const ORB_RULE_FILE: &str = ".claude/skills/poe-crafting-rules/currencies/orb-of-alteration.md";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 114;
const STATISTICS_SEED: u64 = 11_407;
const TEST_POOL_SEED: u64 = 11_408;

/// Items for invariant checks over many results.
const ITEMS: u64 = 2_000;
/// Alterations for the Iron Ring's pool: large enough that a 50:50 choice between prefix and
/// suffix (50 % instead of 52.6 % prefixes with one mod) lies far outside 5 standard
/// deviations, and still done in a few seconds.
const POOL_TRIALS: u64 = 100_000;
/// Alterations per test pool.
const TEST_POOL_TRIALS: u64 = 20_000;

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

/// Files whose text contains `needle`.
fn files_containing(files: &[PathBuf], needle: &str) -> Vec<String> {
    files
        .iter()
        .filter(|path| read_path(path).contains(needle))
        .map(|path| path.display().to_string())
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
/// and not in any file under `web/` (ADR 0011, ADR 0012).
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

/// The lines of the section `## <heading>` of the PO's rules, without the heading.
fn rules_section(rules: &str, heading: &str) -> Vec<String> {
    let (_, after_heading) = rules
        .split_once(&format!("## {heading}\n"))
        .or_else(|| rules.split_once(&format!("## {heading}\r\n")))
        .unwrap_or_else(|| panic!("{CRAFTING_RULES} must have a section `## {heading}`"));
    after_heading
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .map(|line| line.trim_end().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: game data
// ---------------------------------------------------------------------------------------

/// The entry of the currency `id` in `data/currencies.json`.
fn entry_in_file(id: &str) -> Value {
    let content = read_text(CURRENCIES_JSON);
    let data: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    data.get("currencies")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must have an array \"currencies\""))
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {id:?}"))
        .clone()
}

/// The text of the Orb of Alteration's entry in `data/currencies.json` at the given path.
fn text_in_file(path: &[&str]) -> String {
    let entry = entry_in_file(ORB_ID);
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

/// The entry of the currency `id` in `currencies()`.
fn definition(id: &str) -> CurrencyDefinition {
    currencies()
        .into_iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("currencies() must contain {id:?}"))
}

/// Asserts that the Orb of Alteration's message for `code` is `message`, in `currencies()`
/// and in `data/currencies.json`.
fn assert_message_in_game_data(code: &str, message: &str) {
    assert_eq!(
        definition(ORB_ID).messages.get(code).map(String::as_str),
        Some(message),
        "currencies(): message of {ORB_ID:?} for {code:?}"
    );
    assert_eq!(text_in_file(&["messages", code]), message);
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn iron_ring() -> BaseItem {
    base_item(IRON_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer the base item {IRON_RING_ID:?}"))
}

/// The Iron Ring's pool on the page: the valid mods for the UI's item level (ADR 0013).
fn iron_ring_pool() -> Vec<ExplicitModDefinition> {
    valid_mods(&explicit_mods(), UI_ITEM_LEVEL)
}

/// A test mod with one stat. The stat's range is real, so values are rolled. Adapted for
/// AB#167 (PO-confirmed): no Global flag; the level is part of the mod definition, which has
/// no conversion.
fn test_mod(name: &str, affix_type: AffixType, weight: u32, level: u32) -> ExplicitModDefinition {
    let stat_id = format!("{name} stat");
    ExplicitModDefinition {
        name: name.to_owned(),
        affix_type,
        weight,
        definition: ModDefinition {
            family: format!("{name} family"),
            level,
            tags: vec!["test".to_owned()],
            stats: vec![StatDefinition {
                id: stat_id.clone(),
                min: 1,
                max: 10,
            }],
            text: format!("+{{value}} to {name}"),
            placeholders: BTreeMap::from([("value".to_owned(), stat_id)]),
            conversions: BTreeMap::new(),
        },
    }
}

fn rolled(definition: &ExplicitModDefinition, rng: &mut SeededRng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// A magic Iron Ring with a prefix and a suffix that are in none of the pools of these tests,
/// so every mod that is not replaced can be seen.
fn ring_with_old_mods(base: &BaseItem, rng: &mut SeededRng) -> Item {
    let old_prefix = test_mod("Old Prefix", AffixType::Prefix, 1000, 1);
    let old_suffix = test_mod("of the Old Suffix", AffixType::Suffix, 1000, 1);
    let mut item = Item::new(base, rng);
    item.rarity = Rarity::Magic;
    item.prefixes = vec![rolled(&old_prefix, rng)];
    item.suffixes = vec![rolled(&old_suffix, rng)];
    item
}

/// A magic Iron Ring, made with the Orb of Transmutation from `pool`.
fn transmuted_ring(base: &BaseItem, pool: &[ExplicitModDefinition], rng: &mut SeededRng) -> Item {
    let mut item = Item::new(base, rng);
    apply_orb_of_transmutation(&mut item, pool, rng)
        .expect("the Orb of Transmutation must apply to a normal ring");
    assert_eq!(item.rarity, Rarity::Magic);
    item
}

/// Rolled value of the stat with the given identifier.
fn stat_value(rolled: &Mod, id: &str) -> i32 {
    rolled
        .stats
        .iter()
        .find(|stat| stat.id == id)
        .unwrap_or_else(|| panic!("the mod must have a stat {id:?}, found {:?}", rolled.stats))
        .value
}

/// Checks that `rolled` sits in the slot of `slot_type` and was rolled from a definition of
/// `pool`: same name and type, same mod data, every stat value within its range.
fn assert_rolled_from_pool(
    pool: &[ExplicitModDefinition],
    rolled: &ExplicitMod,
    slot_type: AffixType,
    context: &str,
) {
    assert_eq!(
        rolled.affix_type, slot_type,
        "{context}: {:?} sits in the {slot_type:?} slot",
        rolled.name
    );
    let definition = pool
        .iter()
        .find(|definition| definition.name == rolled.name && definition.affix_type == slot_type)
        .unwrap_or_else(|| {
            panic!(
                "{context}: {:?} ({slot_type:?}) must be a new mod from the pool",
                rolled.name
            )
        });
    let expected = &definition.definition;
    let modifier = &rolled.modifier;
    assert_eq!(modifier.family, expected.family, "{context}: family");
    assert_eq!(modifier.tags, expected.tags, "{context}: tags");
    assert_eq!(modifier.text, expected.text, "{context}: text");
    assert_eq!(
        modifier.placeholders, expected.placeholders,
        "{context}: placeholders"
    );
    assert_eq!(
        modifier.stats.len(),
        expected.stats.len(),
        "{context}: stats"
    );
    for stat in &expected.stats {
        let value = stat_value(modifier, &stat.id);
        assert!(
            (stat.min..=stat.max).contains(&value),
            "{context}: {:?} rolled {value} outside {}..={}",
            stat.id,
            stat.min,
            stat.max
        );
    }
}

/// Applies the Orb of Alteration to `item` and checks the invariants of the PO's rule: the orb
/// applies, the item stays a magic Iron Ring with the same implicits, and its prefix and
/// suffix are new mods from `pool`, at least one of them.
fn alter_and_check(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) {
    let base_before = item.base.clone();
    let implicits_before = item.implicits.clone();
    assert_eq!(
        can_apply_orb_of_alteration(item),
        Ok(()),
        "{ctx}: the orb applies to a magic item"
    );
    let result = apply_orb_of_alteration(item, pool, rng);
    assert_eq!(result, Ok(()), "{ctx}: the orb must apply");
    assert_eq!(item.rarity, Rarity::Magic, "{ctx}: the item stays magic");
    assert_eq!(
        item.base, base_before,
        "{ctx}: the base item stays the same"
    );
    assert_eq!(
        item.implicits, implicits_before,
        "{ctx}: the implicits stay unchanged"
    );
    assert!(
        item.prefixes.len() <= 1 && item.suffixes.len() <= 1,
        "{ctx}: a magic item has at most one prefix and one suffix"
    );
    for prefix in &item.prefixes {
        assert_rolled_from_pool(pool, prefix, AffixType::Prefix, ctx);
    }
    for suffix in &item.suffixes {
        assert_rolled_from_pool(pool, suffix, AffixType::Suffix, ctx);
    }
    assert!(
        !item.prefixes.is_empty() || !item.suffixes.is_empty(),
        "{ctx}: the altered item must have at least one mod"
    );
}

fn weight_sum<'a>(definitions: impl IntoIterator<Item = &'a ExplicitModDefinition>) -> f64 {
    definitions
        .into_iter()
        .map(|definition| f64::from(definition.weight))
        .sum()
}

fn of_type(pool: &[ExplicitModDefinition], affix_type: AffixType) -> Vec<&ExplicitModDefinition> {
    pool.iter()
        .filter(|definition| definition.affix_type == affix_type)
        .collect()
}

fn names(pool: &[ExplicitModDefinition]) -> Vec<String> {
    pool.iter()
        .map(|definition| definition.name.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: statistics
// ---------------------------------------------------------------------------------------

/// Outcomes of many Orbs of Alteration.
#[derive(Default)]
struct Tally {
    trials: u64,
    /// Results with exactly one mod.
    one_mod: u64,
    /// Results with a prefix and a suffix.
    two_mods: u64,
    /// Results with one mod: how often each mod was that mod.
    one_mod_by_name: BTreeMap<String, u64>,
    /// Results with one mod: how often it was a prefix.
    one_mod_prefixes: u64,
    /// Results with two mods: how often each prefix and each suffix was chosen.
    two_mod_prefixes: BTreeMap<String, u64>,
    two_mod_suffixes: BTreeMap<String, u64>,
}

impl Tally {
    fn count(map: &BTreeMap<String, u64>, name: &str) -> u64 {
        map.get(name).copied().unwrap_or(0)
    }

    fn add(&mut self, item: &Item) {
        self.trials += 1;
        match (item.prefixes.as_slice(), item.suffixes.as_slice()) {
            ([prefix], [suffix]) => {
                self.two_mods += 1;
                *self
                    .two_mod_prefixes
                    .entry(prefix.name.clone())
                    .or_default() += 1;
                *self
                    .two_mod_suffixes
                    .entry(suffix.name.clone())
                    .or_default() += 1;
            }
            ([only], []) | ([], [only]) => {
                self.one_mod += 1;
                if only.affix_type == AffixType::Prefix {
                    self.one_mod_prefixes += 1;
                }
                *self.one_mod_by_name.entry(only.name.clone()).or_default() += 1;
            }
            ([], []) => panic!("a magic item must have at least one mod"),
            (prefixes, suffixes) => panic!(
                "a magic item has at most one prefix and one suffix, found {} and {}",
                prefixes.len(),
                suffixes.len()
            ),
        }
    }
}

/// Applies the orb `trials` times in a row to the same Iron Ring, as a player does, starting
/// from a ring with an old prefix and an old suffix. Every result is checked with
/// `alter_and_check`, so a mod that is kept instead of replaced fails the test.
fn alter_many(pool: &[ExplicitModDefinition], seed: u64, trials: u64, label: &str) -> Tally {
    let base = iron_ring();
    let mut rng = SeededRng::new(seed);
    let mut item = ring_with_old_mods(&base, &mut rng);
    let mut tally = Tally::default();
    for trial in 0..trials {
        alter_and_check(
            &mut item,
            pool,
            &mut rng,
            &format!("{label}, alteration {trial}"),
        );
        tally.add(&item);
    }
    tally
}

/// Asserts that `count` of `n` lies within 5 standard deviations of the share `p`.
fn assert_share(what: &str, count: u64, n: u64, p: f64) {
    assert!(n > 0, "{what}: no trials to compare");
    let (count_f, n_f) = (count as f64, n as f64);
    let observed = count_f / n_f;
    let tolerance = 5.0 * (p * (1.0 - p) / n_f).sqrt();
    assert!(
        (observed - p).abs() <= tolerance,
        "{what}: observed {observed:.4} ({count} of {n}), expected {p:.4} \u{b1} {tolerance:.4}"
    );
}

/// Asserts the shares of rules 1 to 3 for a pool with prefixes and suffixes: 1 and 2 mods each
/// 50 %; one mod by weight from the whole pool; two mods by weight within each type.
fn assert_rule_shares(pool: &[ExplicitModDefinition], tally: &Tally, label: &str) {
    let prefixes = of_type(pool, AffixType::Prefix);
    let suffixes = of_type(pool, AffixType::Suffix);
    let total = weight_sum(pool);
    let prefix_total = weight_sum(prefixes.iter().copied());
    let suffix_total = weight_sum(suffixes.iter().copied());

    assert_eq!(tally.one_mod + tally.two_mods, tally.trials);
    assert_share(
        &format!("{label}: results with one mod"),
        tally.one_mod,
        tally.trials,
        0.5,
    );
    assert_share(
        &format!("{label}: prefixes among results with one mod"),
        tally.one_mod_prefixes,
        tally.one_mod,
        prefix_total / total,
    );
    for definition in pool {
        assert_share(
            &format!("{label}: {:?} among results with one mod", definition.name),
            Tally::count(&tally.one_mod_by_name, &definition.name),
            tally.one_mod,
            f64::from(definition.weight) / total,
        );
    }
    for definition in &prefixes {
        assert_share(
            &format!(
                "{label}: {:?} among prefixes of results with two mods",
                definition.name
            ),
            Tally::count(&tally.two_mod_prefixes, &definition.name),
            tally.two_mods,
            f64::from(definition.weight) / prefix_total,
        );
    }
    for definition in &suffixes {
        assert_share(
            &format!(
                "{label}: {:?} among suffixes of results with two mods",
                definition.name
            ),
            Tally::count(&tally.two_mod_suffixes, &definition.name),
            tally.two_mods,
            f64::from(definition.weight) / suffix_total,
        );
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Neben dem Orb of Transmutation gibt es einen Orb of Alteration. Sein Name ist ein
// Spieltext."
// (The button's place on the page is checked by the PO in the local preview. Here: both
// currencies exist, the name comes from the game data and is not written into the code, and
// the page uses the new orb from crafting-core.)
#[test]
fn ac1_orb_of_alteration_exists_next_to_transmutation_with_its_game_name() {
    // Given the currencies of crafting-core
    let entries = currencies();

    // When looking for the Orb of Transmutation and the Orb of Alteration
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();

    // Then both exist
    assert!(
        ids.contains(&TRANSMUTATION_ID),
        "currencies() must still contain {TRANSMUTATION_ID:?}, found {ids:?}"
    );
    assert!(
        ids.contains(&ORB_ID),
        "currencies() must contain {ORB_ID:?}, found {ids:?}"
    );
    // And the new orb's id and name come from crafting-core
    assert_eq!(Currency::OrbOfAlteration.id(), ORB_ID);
    assert_eq!(Currency::OrbOfAlteration.name(), ORB_NAME);
    assert_eq!(definition(ORB_ID).name, ORB_NAME);
    // And the name is game data in data/currencies.json
    assert_eq!(text_in_file(&["name"]), ORB_NAME);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(ORB_NAME);
    // And the page shows both orbs from crafting-core
    assert_web_src_uses(
        &[
            "OrbOfTransmutation",
            "OrbOfAlteration",
            "can_apply_orb_of_alteration",
            "apply_orb_of_alteration",
        ],
        "the Orb of Alteration's button comes from crafting-core, next to the Orb of \
         Transmutation",
    );
}

// AC2: „Auf einen magischen Iron Ring angewendet, ersetzt der Orb alle Prefixe und Suffixe
// nach der Regel in `currencies/orb-of-alteration.md`."
// (Covered: rules 1 to 3 with the Iron Ring's pool, with 100 000 alterations instead of the
// PO's 2000, so that a 50:50 choice between prefix and suffix cannot pass within 5 standard
// deviations; a strongly unbalanced test pool; an empty prefix pool and an empty suffix pool;
// rule 4 with an empty pool; the replacement of old mods, also when only one new mod rolls;
// and that the new mods can be the same as the old ones, rolled again.)
// (Adapted for AB#248, PO-confirmed: criterion 3 of AB#248 replaces rule 4. Without a valid
// mod, the orb takes effect: it removes all prefixes and suffixes, adds no mod, and the ring
// stays magic; the orb has no message "Could not generate Mod.". For a magic ring, criterion 3
// gives the same chances as the old rules 1 to 3, so their checks stay.)
#[test]
fn ac2_orb_replaces_all_prefixes_and_suffixes_by_the_rule() {
    let base = iron_ring();

    // --- Rules 1 to 3 with the Iron Ring's pool ---
    // Given magic Iron Rings and the Iron Ring's pool
    let pool = iron_ring_pool();
    assert!(
        !of_type(&pool, AffixType::Prefix).is_empty()
            && !of_type(&pool, AffixType::Suffix).is_empty(),
        "the Iron Ring's pool must have prefixes and suffixes"
    );
    // When the orb is applied many times in a row to the same ring
    let tally = alter_many(&pool, STATISTICS_SEED, POOL_TRIALS, "Iron Ring pool");
    // Then every result follows the rule (checked per result), and the shares follow the
    // weights: 1 and 2 mods each 50 %, one mod from the whole pool, two mods within each type
    assert_rule_shares(&pool, &tally, "Iron Ring pool");

    // --- The old mods are replaced ---
    // Given magic Iron Rings with an old prefix and an old suffix that are not in the pool
    let mut rng = SeededRng::new(SEED);
    let mut one_mod_results = 0;
    for trial in 0..ITEMS {
        let mut item = ring_with_old_mods(&base, &mut rng);
        // When the orb is applied once
        // Then no old mod is left: every mod is a new one from the pool, also when only one
        // mod rolls
        alter_and_check(
            &mut item,
            &pool,
            &mut rng,
            &format!("old mods, item {trial}"),
        );
        if item.prefixes.is_empty() || item.suffixes.is_empty() {
            one_mod_results += 1;
        }
    }
    assert!(
        one_mod_results > 0,
        "some alterations must give one mod, so that the left-out slot is checked"
    );
    // And on rings made magic by the Orb of Transmutation, as on the page, the orb applies too
    for trial in 0..ITEMS {
        let mut item = transmuted_ring(&base, &pool, &mut rng);
        alter_and_check(
            &mut item,
            &pool,
            &mut rng,
            &format!("transmuted ring {trial}"),
        );
    }

    // --- By chance the new mods can be the same as the old ones, rolled again ---
    // Given a pool with one prefix and one suffix, and a ring that has both of them
    let same_prefix = test_mod("Same Prefix", AffixType::Prefix, 1000, 1);
    let same_suffix = test_mod("of the Same Suffix", AffixType::Suffix, 1000, 1);
    let same_pool = vec![same_prefix.clone(), same_suffix.clone()];
    let mut item = Item::new(&base, &mut rng);
    item.rarity = Rarity::Magic;
    item.prefixes = vec![rolled(&same_prefix, &mut rng)];
    item.suffixes = vec![rolled(&same_suffix, &mut rng)];
    let stat_id = &same_prefix.definition.stats[0].id;
    let mut prefix_again = 0;
    let mut value_changed = 0;
    for trial in 0..ITEMS {
        let prefix_before = item.prefixes.first().cloned();
        // When the orb is applied many times in a row
        alter_and_check(
            &mut item,
            &same_pool,
            &mut rng,
            &format!("same mods, {trial}"),
        );
        // Then the old prefix can come again, with a newly rolled value
        if let (Some(before), Some(after)) = (&prefix_before, item.prefixes.first()) {
            prefix_again += 1;
            if stat_value(&before.modifier, stat_id) != stat_value(&after.modifier, stat_id) {
                value_changed += 1;
            }
        }
    }
    assert!(
        prefix_again > 0,
        "the old prefix must be able to roll again (\"by chance the same\")"
    );
    assert!(
        value_changed > 0,
        "a prefix that rolls again is a new mod, so its value is rolled again; in \
         {prefix_again} alterations it never changed"
    );

    // --- A strongly unbalanced pool: prefixes 9000, suffixes 1000 ---
    let prefix_a = test_mod("Test Prefix A", AffixType::Prefix, 6000, 1);
    let prefix_b = test_mod("Test Prefix B", AffixType::Prefix, 3000, 1);
    let suffix_a = test_mod("of Test A", AffixType::Suffix, 600, 1);
    let suffix_b = test_mod("of Test B", AffixType::Suffix, 400, 1);
    let unbalanced = vec![
        prefix_a.clone(),
        prefix_b.clone(),
        suffix_a.clone(),
        suffix_b.clone(),
    ];
    // When the orb is applied many times
    let tally = alter_many(
        &unbalanced,
        TEST_POOL_SEED,
        TEST_POOL_TRIALS,
        "unbalanced pool",
    );
    // Then with one mod, about 90 % are prefixes, not a coin flip; and every share follows the
    // rule
    assert_rule_shares(&unbalanced, &tally, "unbalanced pool");

    // --- An empty prefix pool and an empty suffix pool (rule 3) ---
    let one_sided: [(&str, Vec<ExplicitModDefinition>, AffixType); 4] = [
        (
            "no prefixes, one suffix",
            vec![suffix_a.clone()],
            AffixType::Suffix,
        ),
        (
            "no prefixes, two suffixes",
            vec![suffix_a.clone(), suffix_b.clone()],
            AffixType::Suffix,
        ),
        (
            "no suffixes, one prefix",
            vec![prefix_a.clone()],
            AffixType::Prefix,
        ),
        (
            "no suffixes, two prefixes",
            vec![prefix_a.clone(), prefix_b.clone()],
            AffixType::Prefix,
        ),
    ];
    for (label, one_sided_pool, present) in &one_sided {
        // Given rings with an old prefix and an old suffix
        // When the orb is applied many times in a row
        let tally = alter_many(one_sided_pool, TEST_POOL_SEED, TEST_POOL_TRIALS, label);
        // Then every result has exactly one mod of the present type: the empty type's mod is
        // left out, its old mod is gone, and no second mod of the other type replaces it
        assert_eq!(tally.two_mods, 0, "{label}: no result may have two mods");
        assert_eq!(
            tally.one_mod, tally.trials,
            "{label}: every result has one mod"
        );
        let expected_prefixes = if *present == AffixType::Prefix {
            tally.trials
        } else {
            0
        };
        assert_eq!(
            tally.one_mod_prefixes, expected_prefixes,
            "{label}: the mod is a {present:?}"
        );
        // And the mod is chosen by weight
        let total = weight_sum(one_sided_pool);
        for definition in one_sided_pool {
            assert_share(
                &format!("{label}: {:?}", definition.name),
                Tally::count(&tally.one_mod_by_name, &definition.name),
                tally.trials,
                f64::from(definition.weight) / total,
            );
        }
    }

    // --- No valid mod at all (criterion 3 of AB#248) ---
    // Given magic Iron Rings and a pool without any valid mod
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    for _ in 0..ITEMS {
        for mut item in [
            ring_with_old_mods(&base, &mut rng),
            transmuted_ring(&base, &pool, &mut rng),
        ] {
            let before = item.clone();

            // When the orb is applied
            let result = apply_orb_of_alteration(&mut item, &empty, &mut rng);

            // Then it takes effect: all prefixes and suffixes are removed, no mod is added, and
            // the ring stays magic
            assert_eq!(result, Ok(()));
            assert_eq!(item.rarity, Rarity::Magic);
            assert!(item.prefixes.is_empty() && item.suffixes.is_empty());
            assert_eq!(item.base, before.base);
            assert_eq!(item.implicits, before.implicits);
        }
    }
    // And the Orb of Alteration has no message "Could not generate Mod."
    let messages = definition(ORB_ID).messages;
    assert!(
        messages
            .values()
            .all(|message| message.as_str() != NO_VALID_MOD_MESSAGE),
        "currencies(): {ORB_ID:?} must have no message {NO_VALID_MOD_MESSAGE:?}, found \
         {messages:?}"
    );
}

// AC3: „Ist der Orb anwendbar, zeigt das Überfahren mit der Maus genau den Text "Reforges a
// magic item with new random modifiers"."
// (The tooltip itself is checked by the PO in the local preview. Here: the orb is applicable
// to magic Iron Rings, its description from crafting-core and the game data, and that the
// page asks crafting-core.)
#[test]
fn ac3_applicable_orb_shows_the_game_description_on_hover() {
    // Given magic Iron Rings, with one mod and with two mods
    let base = iron_ring();
    let pool = iron_ring_pool();
    let mut rng = SeededRng::new(SEED);
    let mut seen_one_mod = false;
    let mut seen_two_mods = false;

    for _ in 0..ITEMS {
        let item = transmuted_ring(&base, &pool, &mut rng);
        if !item.prefixes.is_empty() && !item.suffixes.is_empty() {
            seen_two_mods = true;
        } else {
            seen_one_mod = true;
        }

        // When the page asks whether the orb applies
        let applicable = can_apply_orb_of_alteration(&item);

        // Then the orb is applicable
        assert_eq!(applicable, Ok(()), "the orb applies to a magic item");
    }
    assert!(seen_one_mod && seen_two_mods, "rings with one and two mods");

    // And its description is exactly the game text
    assert_eq!(Currency::OrbOfAlteration.description(), ORB_DESCRIPTION);
    // And the text is game data: in currencies() and in data/currencies.json
    assert_eq!(definition(ORB_ID).description, ORB_DESCRIPTION);
    assert_eq!(text_in_file(&["description"]), ORB_DESCRIPTION);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(ORB_DESCRIPTION);
    // And the page asks crafting-core whether the orb applies and takes the description from it
    assert_web_src_uses(
        &["can_apply_orb_of_alteration", "description()"],
        "the orb's tooltip on a magic item is its description from crafting-core",
    );
}

// AC4: „Auf einem normalen Item ist der Orb nicht anwendbar. Überfahren mit der Maus und Klick
// zeigen genau die Meldung "Failed to apply item: Target is not Magic.". Das Item bleibt
// unverändert."
// (Tooltip and message area are checked by the PO in the local preview. Here: the reason on
// hover and on click, its message from crafting-core and the game data, the unchanged item,
// and that the page marks the orb and shows the message.)
#[test]
fn ac4_orb_on_a_normal_item_shows_the_game_message_and_keeps_the_item() {
    // Given normal Iron Rings, the Iron Ring's pool and an empty pool
    let base = iron_ring();
    let pool = iron_ring_pool();
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let mut rng = SeededRng::new(SEED);

    for _ in 0..ITEMS {
        for click_pool in [&pool, &empty] {
            let mut item = Item::new(&base, &mut rng);
            assert_eq!(item.rarity, Rarity::Normal);
            let before = item.clone();

            // When the page asks whether the orb applies (hover)
            let hover = can_apply_orb_of_alteration(&item);
            // And the orb is clicked anyway
            let click = apply_orb_of_alteration(&mut item, click_pool, &mut rng);

            // Then both fail because the item is not magic, whatever the pool
            assert_eq!(hover, Err(CraftError::ItemNotMagic));
            assert_eq!(click, Err(CraftError::ItemNotMagic));
            // And both show exactly the game message
            for error in [hover, click] {
                let error = error.expect_err("the orb does not apply to a normal item");
                assert_eq!(
                    Currency::OrbOfAlteration.failure_message(error),
                    NOT_MAGIC_MESSAGE
                );
            }
            // And the item is unchanged
            assert_eq!(item, before);
        }
    }
    // And the reason code links the error to the game data
    assert_eq!(CraftError::ItemNotMagic.code(), NOT_MAGIC_CODE);
    assert_message_in_game_data(NOT_MAGIC_CODE, NOT_MAGIC_MESSAGE);
    // And the text is not written into the code of crafting-core or web
    assert_only_in_game_data(NOT_MAGIC_MESSAGE);
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &[
            "can_apply_orb_of_alteration",
            "apply_orb_of_alteration",
            "failure_message",
            "aria-disabled",
        ],
        "an inapplicable Orb of Alteration is marked and shows its failure message",
    );
}

// AC5: „`poe-crafting-rules`: Die neue Datei `currencies/orb-of-alteration.md` hat genau den
// Inhalt unten und wird in der Liste "Currency rules" verlinkt."
// (The lines that AB#248 does not replace are quoted in `ORB_RULE_LINES`.)
// (Adapted for AB#248, PO-confirmed: criterion 3 of AB#248 replaces the "Effect" line, rules 1
// to 4 and the sentence on the empty pool in the evidence. Their wording is checked at the
// acceptance of AB#248, so the test checks only that the file starts with the other lines of
// the PO's content, and the link.)
#[test]
fn ac5_alteration_rule_file_has_the_po_content_and_is_linked() {
    // Given the PO's rules and the rule file for the Orb of Alteration
    let rules = read_text(CRAFTING_RULES);
    let orb_rules = read_text(ORB_RULE_FILE);

    // When reading the file line by line
    let file_lines: Vec<&str> = orb_rules.lines().map(str::trim_end).collect();

    // Then it starts with exactly the PO's lines that AB#248 does not replace
    assert!(
        file_lines.starts_with(&ORB_RULE_LINES),
        "{ORB_RULE_FILE} must start with the PO's lines {ORB_RULE_LINES:#?}, found {:#?}",
        file_lines
            .iter()
            .take(ORB_RULE_LINES.len())
            .collect::<Vec<_>>()
    );

    // And the list "Currency rules" in the section "Currencies" links the file
    let currencies_section = rules_section(&rules, "Currencies");
    let list: Vec<&String> = currencies_section
        .iter()
        .skip_while(|line| line.as_str() != CURRENCY_RULES_HEADING)
        .skip(1)
        .take_while(|line| line.starts_with("- "))
        .collect();
    assert!(
        !list.is_empty(),
        "the section `## Currencies` of {CRAFTING_RULES} must have the list \
         {CURRENCY_RULES_HEADING:?}"
    );
    assert!(
        list.iter()
            .any(|line| line.contains(&format!("]({ORB_RULE_LINK})"))),
        "the list {CURRENCY_RULES_HEADING:?} in {CRAFTING_RULES} must link {ORB_RULE_LINK}, \
         found {list:#?}"
    );
}

// AC6: „`poe-crafting-rules`, Abschnitt "Item level": Die Zeile "TODO (PO): how item level
// restricts which mods can roll. Default in the UI: 100." wird ersetzt durch genau:
//    A mod can roll on an item only if the item level is at least the mod's level. Example: a
//    mod with level 60 needs an item level of 60 or higher. Default in the UI: 100.
//    - Evidence: the PO's knowledge of the game. Consistent with the PO's measurement on
//    2026-10-21 (3000 Orbs of Transmutation and Alteration on a Bone Ring of item level 83):
//    mods with level 82 rolled 63 times, mods with level 84 or 85 never."
// (Besides the text, the rule is checked where the tech lead's plan and ADR 0013 put it:
// `valid_mods`, the pool both orbs roll from, and the page building its pool with it.)
// (Adapted for AB#185, PO-confirmed: criterion 6 of AB#185 gives every base its own pool
// (ADR 0017). The page builds its pool from `mod_pool` instead of `explicit_mods`.)
// (Adapted for AB#248, PO-confirmed: criterion 4 of AB#248 replaces the check that the page
// builds the pool with `valid_mods`. The page passes the base's pool from `mod_pool`, and the
// orbs apply the item level rule themselves (ADR 0021, `story_248::ac4`). The rest stays.)
#[test]
fn ac6_item_level_rule_is_written_down_and_decides_the_valid_mods() {
    // --- The PO's text ---
    // Given the PO's rules
    let rules = read_text(CRAFTING_RULES);
    // When reading the section "Item level"
    let section = rules_section(&rules, "Item level");
    // Then it contains the PO's rule, directly followed by its evidence
    let position = section
        .iter()
        .position(|line| line == ITEM_LEVEL_RULE)
        .unwrap_or_else(|| {
            panic!(
                "the section `## Item level` of {CRAFTING_RULES} must contain {ITEM_LEVEL_RULE:?}"
            )
        });
    assert_eq!(
        section.get(position + 1).map(String::as_str),
        Some(ITEM_LEVEL_EVIDENCE),
        "in {CRAFTING_RULES}, the evidence must follow the item level rule directly"
    );
    // And the TODO line is gone
    assert!(
        !rules.contains(ITEM_LEVEL_TODO),
        "{CRAFTING_RULES} must no longer contain {ITEM_LEVEL_TODO:?}"
    );

    // --- The rule decides the valid mods ---
    // Given mods of different levels, in no particular order
    let levels = [60, 1, 85, 59, 83, 61, 100, 82, 84, 101];
    let mods: Vec<ExplicitModDefinition> = levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let affix_type = if index % 2 == 0 {
                AffixType::Prefix
            } else {
                AffixType::Suffix
            };
            test_mod(&format!("Level {level}"), affix_type, 1000, *level)
        })
        .collect();
    let expected = |item_level: u32| -> Vec<String> {
        mods.iter()
            .filter(|definition| definition.definition.level <= item_level)
            .map(|definition| definition.name.clone())
            .collect()
    };
    for item_level in [0, 1, 58, 59, 60, 61, 82, 83, 84, 85, 99, 100, 101, 200] {
        // When asking for the valid mods at an item level
        let valid = valid_mods(&mods, item_level);
        // Then they are exactly the mods whose level is at most the item level, in their order,
        // unchanged
        assert_eq!(
            names(&valid),
            expected(item_level),
            "valid mods at item level {item_level}"
        );
        for definition in &valid {
            let original = mods
                .iter()
                .find(|original| original.name == definition.name)
                .expect("a valid mod comes from the given mods");
            assert_eq!(definition, original, "valid mods are passed on unchanged");
        }
    }
    // And the PO's example holds: a mod with level 60 needs an item level of 60 or higher
    for (item_level, allowed) in [(59, false), (60, true), (61, true), (100, true)] {
        let valid = names(&valid_mods(&mods, item_level));
        assert_eq!(
            valid.contains(&"Level 60".to_owned()),
            allowed,
            "the mod with level 60 at item level {item_level}"
        );
    }

    // --- Both orbs roll only valid mods (the PO's measurement at item level 83) ---
    // Given a ring of item level 83 and mods with level 82 to 85
    let base = iron_ring();
    let measured = vec![
        test_mod("Level 82 Prefix", AffixType::Prefix, 1000, 82),
        test_mod("Level 85 Prefix", AffixType::Prefix, 1000, 85),
        test_mod("of Level 83", AffixType::Suffix, 1000, 83),
        test_mod("of Level 84", AffixType::Suffix, 1000, 84),
    ];
    let pool_at_83 = valid_mods(&measured, 83);
    let mut rng = SeededRng::new(SEED);
    let mut rolled_names: BTreeMap<String, u64> = BTreeMap::new();
    for trial in 0..ITEMS {
        // When the Orb of Transmutation and then the Orb of Alteration are applied
        let mut item = transmuted_ring(&base, &pool_at_83, &mut rng);
        for rolled in item.prefixes.iter().chain(item.suffixes.iter()) {
            *rolled_names.entry(rolled.name.clone()).or_default() += 1;
        }
        alter_and_check(
            &mut item,
            &pool_at_83,
            &mut rng,
            &format!("item level 83, {trial}"),
        );
        for rolled in item.prefixes.iter().chain(item.suffixes.iter()) {
            *rolled_names.entry(rolled.name.clone()).or_default() += 1;
        }
    }
    // Then mods with level 82 and 83 roll, mods with level 84 or 85 never
    for (name, rolls) in [
        ("Level 82 Prefix", true),
        ("of Level 83", true),
        ("Level 85 Prefix", false),
        ("of Level 84", false),
    ] {
        assert_eq!(
            rolled_names.get(name).copied().unwrap_or(0) > 0,
            rolls,
            "{name:?} at item level 83, rolled: {rolled_names:?}"
        );
    }

    // --- The Iron Ring's pool at the UI's default item level ---
    // Given the mods of crafting-core
    let all = explicit_mods();
    // When asking for the valid mods at several item levels, including the UI default of 100
    for item_level in [0, 1, 2, 3, 7, 8, 15, 16, UI_ITEM_LEVEL] {
        let expected: Vec<String> = all
            .iter()
            .filter(|definition| definition.definition.level <= item_level)
            .map(|definition| definition.name.clone())
            .collect();
        // Then they follow the rule
        assert_eq!(
            names(&valid_mods(&all, item_level)),
            expected,
            "valid mods of crafting-core at item level {item_level}"
        );
    }
    // And the page takes the pool for both orbs from crafting-core
    assert_web_src_uses(
        &["mod_pool"],
        "the page takes the pool for both orbs from crafting-core",
    );
}
