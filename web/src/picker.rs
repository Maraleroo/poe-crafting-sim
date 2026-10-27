//! The base picker: whether it is open, on which level it is, what has been typed, and what it
//! shows for that.
//!
//! This is plain Rust so it can be tested without a browser (ADR 0010, ADR 0018). Slots, their
//! bases, class names and implicit texts come from `crafting-core`; UI texts come from the
//! translations. Matching and ordering of suggestions are UI behaviour and live here.

use crate::i18n::Translations;
use crafting_core::Slot;

/// The least number of letters that give suggestions.
const MIN_QUERY_LENGTH: usize = 2;

/// The slots in the order in which the grid shows them, row by row: weapon, helmet and
/// off-hand, then ring, body armour and amulet, then gloves, belt and boots.
const GRID: [Slot; 9] = [
    Slot::Weapon,
    Slot::Helmet,
    Slot::OffHand,
    Slot::Ring,
    Slot::BodyArmour,
    Slot::Amulet,
    Slot::Gloves,
    Slot::Belt,
    Slot::Boots,
];

/// A base of the catalog, with the game texts needed to show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseOption {
    pub id: String,
    pub name: String,
    pub item_class: String,
    /// The lines of the implicit texts with value ranges (`range_text`); none for a base
    /// without an implicit.
    pub implicit: Vec<String>,
}

/// A slot of the catalog: its item classes and the bases it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotData {
    pub slot: Slot,
    pub item_classes: Vec<String>,
    pub bases: Vec<BaseOption>,
}

impl SlotData {
    /// A slot is available when it has bases.
    fn available(&self) -> bool {
        !self.bases.is_empty()
    }
}

/// Everything the picker offers, read once from `crafting-core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    slots: Vec<SlotData>,
}

impl Catalog {
    /// The slots, classes and bases of `crafting-core`.
    pub fn new() -> Self {
        Self::from_slots(
            Slot::ALL
                .iter()
                .map(|slot| SlotData {
                    slot: *slot,
                    item_classes: slot.item_classes(),
                    bases: slot
                        .base_items()
                        .into_iter()
                        .map(|base| BaseOption {
                            id: base.id,
                            name: base.name,
                            item_class: base.item_class,
                            implicit: base
                                .implicits
                                .iter()
                                .flat_map(|implicit| {
                                    implicit
                                        .range_text()
                                        .lines()
                                        .map(str::to_owned)
                                        .collect::<Vec<_>>()
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        )
    }

    /// A catalog of the given slots, in the order of `Slot::ALL`.
    pub fn from_slots(slots: Vec<SlotData>) -> Self {
        Self { slots }
    }

    fn data(&self, slot: Slot) -> Option<&SlotData> {
        self.slots.iter().find(|data| data.slot == slot)
    }

    fn available(&self, slot: Slot) -> bool {
        self.data(slot).is_some_and(SlotData::available)
    }

    fn has_base(&self, id: &str) -> bool {
        self.slots
            .iter()
            .any(|data| data.bases.iter().any(|base| base.id == id))
    }
}

/// The level of the picker: the slots of all items, or the bases of one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    AllItems,
    Slot(Slot),
}

/// What a click on a button of the picker does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Go to a level.
    Go(Level),
    /// Choose the base with this identifier.
    Choose(String),
}

impl Action {
    /// A text that stands for the action, to keep it on the button of the page.
    pub fn token(&self) -> String {
        match self {
            Action::Go(Level::AllItems) => "go".to_owned(),
            Action::Go(Level::Slot(slot)) => format!("go:{}", slot.id()),
            Action::Choose(id) => format!("choose:{id}"),
        }
    }

    /// The action for a text from [`Action::token`]; none if the text stands for no action.
    pub fn from_token(token: &str) -> Option<Self> {
        if token == "go" {
            return Some(Action::Go(Level::AllItems));
        }
        if let Some(slot_id) = token.strip_prefix("go:") {
            let slot = Slot::ALL.iter().find(|slot| slot.id() == slot_id)?;
            return Some(Action::Go(Level::Slot(*slot)));
        }
        token
            .strip_prefix("choose:")
            .map(|id| Action::Choose(id.to_owned()))
    }
}

/// The state of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    open: bool,
    level: Level,
    query: String,
}

impl Picker {
    /// A closed picker on the first level, with nothing typed.
    pub fn new() -> Self {
        Self {
            open: false,
            level: Level::AllItems,
            query: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the picker; the level and the typed text stay.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Closes the picker and forgets the level and the typed text.
    pub fn close(&mut self) {
        *self = Self::new();
    }

    /// The text of the field.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The text of the field has changed.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
    }

    /// Does what a click on a button does. A step to a level clears the typed text. Choosing a
    /// base closes the picker and returns the identifier of the base; an unknown base or a slot
    /// that is not available changes nothing.
    pub fn apply(&mut self, catalog: &Catalog, action: &Action) -> Option<String> {
        match action {
            Action::Go(Level::Slot(slot)) if !catalog.available(*slot) => None,
            Action::Go(level) => {
                self.level = *level;
                self.query.clear();
                None
            }
            Action::Choose(id) if catalog.has_base(id) => {
                self.close();
                Some(id.clone())
            }
            Action::Choose(_) => None,
        }
    }

    /// What the picker shows now.
    pub fn view(&self, catalog: &Catalog, translations: &Translations) -> PickerView {
        let body = if self.has_suggestions() {
            self.suggestions(catalog, translations)
        } else {
            match self.level {
                Level::AllItems => slot_tiles(catalog, translations),
                Level::Slot(slot) => base_options(catalog, slot, translations),
            }
        };
        PickerView {
            path: self.path(translations),
            separator: translations.text("picker.path.separator").to_owned(),
            body,
            hint: translations.text("picker.hint").to_owned(),
        }
    }

    fn has_suggestions(&self) -> bool {
        self.query.trim().chars().count() >= MIN_QUERY_LENGTH
    }

    /// The path: the first part, then the slot if the picker is on its level. Every part
    /// except the last one is a button.
    fn path(&self, translations: &Translations) -> Vec<PathPart> {
        let root = translations.text("picker.path.root").to_owned();
        match self.level {
            Level::AllItems => vec![PathPart {
                label: root,
                action: None,
            }],
            Level::Slot(slot) => vec![
                PathPart {
                    label: root,
                    action: Some(Action::Go(Level::AllItems)),
                },
                PathPart {
                    label: slot_name(slot, translations),
                    action: None,
                },
            ],
        }
    }

    /// The classes whose name contains the typed text, then the bases whose name contains it,
    /// without regard to case.
    fn suggestions(&self, catalog: &Catalog, translations: &Translations) -> Body {
        let needle = self.query.trim().to_lowercase();
        let mut options = Vec::new();
        for data in catalog.slots.iter().filter(|data| data.available()) {
            for class in &data.item_classes {
                if class.to_lowercase().contains(&needle) {
                    options.push(OptionView {
                        kind: Some(translations.text("picker.kind.class").to_owned()),
                        label: class.clone(),
                        implicit: Vec::new(),
                        action: Action::Go(Level::Slot(data.slot)),
                    });
                }
            }
        }
        for data in &catalog.slots {
            for base in &data.bases {
                if base.name.to_lowercase().contains(&needle) {
                    options.push(OptionView {
                        kind: Some(translations.text("picker.kind.base").to_owned()),
                        label: translations.format(
                            "picker.base_label",
                            &[("base", &base.name), ("class", &base.item_class)],
                        ),
                        implicit: base.implicit.clone(),
                        action: Action::Choose(base.id.clone()),
                    });
                }
            }
        }
        if options.is_empty() {
            Body::NothingFound(translations.text("picker.nothing_found").to_owned())
        } else {
            Body::Options(options)
        }
    }
}

/// The name of a slot, a UI text.
fn slot_name(slot: Slot, translations: &Translations) -> String {
    translations
        .text(&format!("picker.slot.{}", slot.id()))
        .to_owned()
}

/// The nine slots in the order of the grid; a slot without bases has a note and no action.
fn slot_tiles(catalog: &Catalog, translations: &Translations) -> Body {
    Body::Slots(
        GRID.iter()
            .map(|slot| {
                let available = catalog.available(*slot);
                SlotTile {
                    label: slot_name(*slot, translations),
                    action: available.then_some(Action::Go(Level::Slot(*slot))),
                    note: (!available)
                        .then(|| translations.text("picker.not_available").to_owned()),
                }
            })
            .collect(),
    )
}

/// The bases of a slot in the order of the data.
fn base_options(catalog: &Catalog, slot: Slot, translations: &Translations) -> Body {
    let options: Vec<OptionView> = catalog
        .data(slot)
        .map(|data| {
            data.bases
                .iter()
                .map(|base| OptionView {
                    kind: None,
                    label: base.name.clone(),
                    implicit: base.implicit.clone(),
                    action: Action::Choose(base.id.clone()),
                })
                .collect()
        })
        .unwrap_or_default();
    if options.is_empty() {
        Body::NothingFound(translations.text("picker.nothing_found").to_owned())
    } else {
        Body::Options(options)
    }
}

/// What the picker shows: the path, a body and the hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerView {
    pub path: Vec<PathPart>,
    /// The text between two parts of the path.
    pub separator: String,
    pub body: Body,
    pub hint: String,
}

/// A part of the path. It is a button if it has an action; only the last part has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPart {
    pub label: String,
    pub action: Option<Action>,
}

/// The main part of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// The slots of the grid.
    Slots(Vec<SlotTile>),
    /// Bases, or classes and bases.
    Options(Vec<OptionView>),
    /// The text shown when there is nothing to choose.
    NothingFound(String),
}

/// A slot in the grid. Without an action it cannot be chosen and shows a note instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotTile {
    pub label: String,
    pub action: Option<Action>,
    pub note: Option<String>,
}

/// A class or a base to choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionView {
    /// The mark of a suggestion: class or base. A list of bases has none.
    pub kind: Option<String>,
    pub label: String,
    /// The lines of the implicit text of a base.
    pub implicit: Vec<String>,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(id: &str, name: &str, item_class: &str, implicit: &[&str]) -> BaseOption {
        BaseOption {
            id: id.to_owned(),
            name: name.to_owned(),
            item_class: item_class.to_owned(),
            implicit: implicit.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    /// Invented data: the ring slot holds three bases of the class "Zorbs" and one of the class
    /// "Quibs", the amulet slot one base of the class "Wands", every other slot is empty.
    fn catalog() -> Catalog {
        Catalog::from_slots(
            Slot::ALL
                .iter()
                .map(|slot| SlotData {
                    slot: *slot,
                    item_classes: match slot {
                        Slot::Ring => vec!["Zorbs".to_owned(), "Quibs".to_owned()],
                        Slot::Amulet => vec!["Wands".to_owned()],
                        _ => Vec::new(),
                    },
                    bases: match slot {
                        Slot::Ring => vec![
                            base("z1", "Plain Zorb", "Zorbs", &[]),
                            base("z2", "Twin Zorb", "Zorbs", &["+(1-2) to a"]),
                            base("z3", "Twin Zorb", "Zorbs", &["+(3-4) to a", "second line"]),
                            base("q1", "Quib", "Quibs", &[]),
                        ],
                        Slot::Amulet => vec![base("w1", "Zorb Wand", "Wands", &[])],
                        _ => Vec::new(),
                    },
                })
                .collect(),
        )
    }

    fn translations() -> Translations {
        Translations::english()
    }

    fn view_of(picker: &Picker) -> PickerView {
        picker.view(&catalog(), &translations())
    }

    fn typed(query: &str) -> Picker {
        let mut picker = Picker::new();
        picker.open();
        picker.set_query(query);
        picker
    }

    fn options(view: &PickerView) -> &[OptionView] {
        match &view.body {
            Body::Options(options) => options,
            other => panic!("expected options, found {other:?}"),
        }
    }

    fn labels(options: &[OptionView]) -> Vec<&str> {
        options.iter().map(|option| option.label.as_str()).collect()
    }

    #[test]
    fn the_picker_starts_closed_on_the_first_level_with_nothing_typed() {
        let picker = Picker::new();
        assert!(!picker.is_open());
        assert_eq!(picker.level, Level::AllItems);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn open_keeps_the_text_and_close_forgets_level_and_text() {
        let mut picker = typed("zo");
        picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Ring)));
        picker.set_query("tw");
        picker.close();
        assert_eq!(picker, Picker::new());
        picker.open();
        assert!(picker.is_open());
    }

    #[test]
    fn the_first_level_shows_the_nine_slots_in_the_order_of_the_grid() {
        let t = translations();
        let view = view_of(&typed(""));
        let Body::Slots(tiles) = &view.body else {
            panic!("expected the slots, found {:?}", view.body);
        };
        let expected: Vec<&str> = GRID
            .iter()
            .map(|slot| t.text(&format!("picker.slot.{}", slot.id())))
            .collect();
        let labels: Vec<&str> = tiles.iter().map(|tile| tile.label.as_str()).collect();
        assert_eq!(labels, expected);
    }

    #[test]
    fn the_grid_holds_every_slot_once() {
        let mut slots = GRID.to_vec();
        slots.sort_by_key(|slot| slot.id());
        let mut all = Slot::ALL.to_vec();
        all.sort_by_key(|slot| slot.id());
        assert_eq!(slots, all);
    }

    #[test]
    fn only_a_slot_with_bases_can_be_chosen_and_the_others_say_so() {
        let t = translations();
        let view = view_of(&typed(""));
        let Body::Slots(tiles) = &view.body else {
            panic!("expected the slots, found {:?}", view.body);
        };
        let catalog = catalog();
        assert_eq!(tiles.len(), GRID.len());
        for (slot, tile) in GRID.iter().zip(tiles) {
            if catalog.available(*slot) {
                assert_eq!(tile.action, Some(Action::Go(Level::Slot(*slot))));
                assert_eq!(tile.note, None);
            } else {
                assert_eq!(tile.action, None, "{}", tile.label);
                assert_eq!(
                    tile.note.as_deref(),
                    Some(t.text("picker.not_available")),
                    "{}",
                    tile.label
                );
            }
        }
        assert!(catalog.available(Slot::Ring) && !catalog.available(Slot::Weapon));
    }

    #[test]
    fn a_slot_without_bases_cannot_be_entered() {
        let mut picker = Picker::new();
        assert_eq!(
            picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Boots))),
            None
        );
        assert_eq!(picker.level, Level::AllItems);
    }

    #[test]
    fn a_slot_lists_its_bases_in_the_order_of_the_data_with_their_implicits() {
        let mut picker = typed("");
        picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Ring)));
        let view = view_of(&picker);
        let list = options(&view);
        assert_eq!(
            labels(list),
            ["Plain Zorb", "Twin Zorb", "Twin Zorb", "Quib"]
        );
        assert!(list.iter().all(|option| option.kind.is_none()));
        assert!(list[0].implicit.is_empty());
        assert_eq!(list[1].implicit, ["+(1-2) to a"]);
        assert_eq!(list[2].implicit, ["+(3-4) to a", "second line"]);
        assert_eq!(list[2].action, Action::Choose("z3".to_owned()));
    }

    #[test]
    fn the_path_starts_with_the_first_part_and_only_the_last_part_is_no_button() {
        let t = translations();
        let mut picker = typed("");
        let view = view_of(&picker);
        assert_eq!(
            view.path,
            [PathPart {
                label: t.text("picker.path.root").to_owned(),
                action: None
            }]
        );

        picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Ring)));
        let view = view_of(&picker);
        assert_eq!(
            view.path,
            [
                PathPart {
                    label: t.text("picker.path.root").to_owned(),
                    action: Some(Action::Go(Level::AllItems))
                },
                PathPart {
                    label: t.text("picker.slot.ring").to_owned(),
                    action: None
                }
            ]
        );
        assert_eq!(view.separator, t.text("picker.path.separator"));
    }

    #[test]
    fn a_click_on_an_earlier_part_of_the_path_goes_back() {
        let mut picker = typed("");
        picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Ring)));
        let back = view_of(&picker).path[0].action.clone().expect("a button");
        picker.apply(&catalog(), &back);
        assert_eq!(picker.level, Level::AllItems);
        assert!(matches!(view_of(&picker).body, Body::Slots(_)));
    }

    #[test]
    fn no_text_or_one_letter_gives_no_suggestions() {
        for query in ["", "z", " z ", "   "] {
            let view = view_of(&typed(query));
            assert!(matches!(view.body, Body::Slots(_)), "{query:?}");
        }
    }

    #[test]
    fn two_letters_give_suggestions_with_the_classes_first() {
        let view = view_of(&typed("zo"));
        let list = options(&view);
        assert_eq!(
            labels(list),
            [
                "Zorbs",
                "Zorb Wand (Wands)",
                "Plain Zorb (Zorbs)",
                "Twin Zorb (Zorbs)",
                "Twin Zorb (Zorbs)"
            ]
        );
        let t = translations();
        assert_eq!(list[0].kind.as_deref(), Some(t.text("picker.kind.class")));
        assert!(list[1..]
            .iter()
            .all(|option| option.kind.as_deref() == Some(t.text("picker.kind.base"))));
    }

    #[test]
    fn the_spaces_around_the_text_do_not_count() {
        let padded = view_of(&typed("  zo "));
        let plain = view_of(&typed("zo"));
        assert_eq!(padded.body, plain.body);
        assert!(matches!(view_of(&typed(" z ")).body, Body::Slots(_)));
    }

    #[test]
    fn matching_ignores_case_and_looks_at_any_part_of_the_name() {
        let view = view_of(&typed("TWIN"));
        assert_eq!(labels(options(&view)), ["Twin Zorb (Zorbs)"; 2]);
        let view = view_of(&typed("ib"));
        assert_eq!(labels(options(&view)), ["Quibs", "Quib (Quibs)"]);
    }

    #[test]
    fn the_implicit_text_is_shown_but_not_searched() {
        let view = view_of(&typed("to a"));
        assert_eq!(
            view.body,
            Body::NothingFound(translations().text("picker.nothing_found").to_owned())
        );
        let view = view_of(&typed("twin"));
        let list = options(&view);
        assert_eq!(list[0].implicit, ["+(1-2) to a"]);
        assert_eq!(list[1].implicit, ["+(3-4) to a", "second line"]);
    }

    #[test]
    fn without_a_match_the_picker_says_nothing_was_found() {
        let view = view_of(&typed("xyz"));
        assert_eq!(
            view.body,
            Body::NothingFound(translations().text("picker.nothing_found").to_owned())
        );
    }

    #[test]
    fn suggestions_replace_the_list_on_a_lower_level() {
        let mut picker = typed("");
        picker.apply(&catalog(), &Action::Go(Level::Slot(Slot::Ring)));
        picker.set_query("wand");
        let view = view_of(&picker);
        assert_eq!(labels(options(&view)), ["Wands", "Zorb Wand (Wands)"]);
        assert_eq!(view.path.len(), 2);
    }

    #[test]
    fn a_class_suggestion_opens_the_slot_of_the_class_and_clears_the_text() {
        let mut picker = typed("zo");
        let action = options(&view_of(&picker))[0].action.clone();
        assert_eq!(action, Action::Go(Level::Slot(Slot::Ring)));
        assert_eq!(picker.apply(&catalog(), &action), None);
        assert!(picker.is_open());
        assert_eq!(picker.query(), "");
        let view = view_of(&picker);
        assert_eq!(options(&view).len(), 4);
    }

    #[test]
    fn choosing_a_base_closes_the_picker_and_returns_the_base() {
        let mut picker = typed("zo");
        let action = options(&view_of(&picker))[4].action.clone();
        assert_eq!(picker.apply(&catalog(), &action), Some("z3".to_owned()));
        assert_eq!(picker, Picker::new());
    }

    #[test]
    fn an_unknown_base_is_not_chosen() {
        let mut picker = typed("zo");
        let before = picker.clone();
        assert_eq!(
            picker.apply(&catalog(), &Action::Choose("unknown".to_owned())),
            None
        );
        assert_eq!(picker, before);
    }

    #[test]
    fn an_action_survives_its_token() {
        let mut actions = vec![
            Action::Go(Level::AllItems),
            Action::Choose("some/id:with:colons".to_owned()),
        ];
        actions.extend(Slot::ALL.iter().map(|slot| Action::Go(Level::Slot(*slot))));
        for action in actions {
            assert_eq!(Action::from_token(&action.token()), Some(action));
        }
        assert_eq!(Action::from_token("go:unknown"), None);
        assert_eq!(Action::from_token("other"), None);
    }

    #[test]
    fn the_view_carries_the_hint() {
        let view = view_of(&Picker::new());
        assert_eq!(view.hint, translations().text("picker.hint"));
    }

    #[test]
    fn the_catalog_of_the_game_data_offers_every_base_once() {
        let catalog = Catalog::new();
        let in_slots: usize = catalog.slots.iter().map(|data| data.bases.len()).sum();
        assert_eq!(in_slots, crafting_core::base_items().len());
        assert_eq!(catalog.slots.len(), Slot::ALL.len());
        assert!(catalog.available(Slot::Ring));
        assert!(!catalog.available(Slot::Boots));
    }

    #[test]
    fn the_catalog_tells_bases_with_the_same_name_apart_by_their_implicit_text() {
        let catalog = Catalog::new();
        let mut seen = std::collections::BTreeSet::new();
        for base in catalog.slots.iter().flat_map(|data| &data.bases) {
            assert!(
                seen.insert((base.name.clone(), base.implicit.clone())),
                "{}",
                base.id
            );
        }
    }

    #[test]
    fn every_text_of_the_picker_has_a_translation() {
        let t = translations();
        for key in [
            "picker.field",
            "picker.placeholder",
            "picker.path.root",
            "picker.path.separator",
            "picker.not_available",
            "picker.nothing_found",
            "picker.kind.class",
            "picker.kind.base",
            "picker.hint",
            "picker.base_label",
        ] {
            assert!(!t.text(key).is_empty(), "{key}");
        }
        for slot in Slot::ALL {
            assert!(!slot_name(slot, &t).is_empty(), "{}", slot.id());
        }
    }

    #[test]
    fn the_base_label_puts_the_class_in_brackets() {
        let t = translations();
        assert_eq!(
            t.format("picker.base_label", &[("base", "A"), ("class", "B")]),
            "A (B)"
        );
    }

    #[test]
    fn the_real_class_is_found_by_its_own_name() {
        let catalog = Catalog::new();
        let class = Slot::Ring.item_classes().remove(0);
        let mut picker = Picker::new();
        picker.open();
        picker.set_query(&class.to_lowercase());
        let view = picker.view(&catalog, &translations());
        let first = &options(&view)[0];
        assert_eq!(first.label, class);
        assert_eq!(first.action, Action::Go(Level::Slot(Slot::Ring)));
    }
}
