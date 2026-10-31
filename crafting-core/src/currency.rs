//! Currencies: the game data entries and the currencies the crafting rules implement.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::craft::CraftError;
use crate::data::currencies;

/// A currency in the game data (ADR 0011).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CurrencyDefinition {
    /// Project identifier, e.g. `orb_of_transmutation`. Not a game value.
    pub id: String,
    /// English game text, passed through unchanged.
    pub name: String,
    /// English game description, passed through unchanged (ADR 0012).
    pub description: String,
    /// The game's failure message for each reason code (`CraftError::code`), passed through
    /// unchanged (ADR 0012).
    pub messages: BTreeMap<String, String>,
}

/// A currency with crafting rules in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    OrbOfTransmutation,
    OrbOfAlteration,
}

impl Currency {
    /// The identifier of the currency's entry in the game data.
    pub fn id(self) -> &'static str {
        match self {
            Currency::OrbOfTransmutation => "orb_of_transmutation",
            Currency::OrbOfAlteration => "orb_of_alteration",
        }
    }

    /// The name of the currency from the game data.
    ///
    /// # Panics
    /// If the game data has no entry for the currency. That is a programming error, caught
    /// by the unit tests.
    pub fn name(self) -> String {
        self.definition().name
    }

    /// The description of the currency from the game data.
    ///
    /// # Panics
    /// If the game data has no entry for the currency. That is a programming error, caught
    /// by the unit tests.
    pub fn description(self) -> String {
        self.definition().description
    }

    /// The game's message for a failure of this currency, from the game data.
    ///
    /// # Panics
    /// If the game data has no entry for the currency or no message for the reason. That is
    /// a programming error, caught by the unit tests.
    pub fn failure_message(self, error: CraftError) -> String {
        let id = self.id();
        let code = error.code();
        self.definition()
            .messages
            .remove(code)
            .unwrap_or_else(|| panic!("the game data has no message of {id:?} for {code:?}"))
    }

    fn definition(self) -> CurrencyDefinition {
        let id = self.id();
        currencies()
            .into_iter()
            .find(|currency| currency.id == id)
            .unwrap_or_else(|| panic!("the game data has no currency {id:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant of [`Currency`]. The exhaustive match makes a new variant a compile
    /// error here until it is added to the list.
    fn all() -> Vec<Currency> {
        let variants = [Currency::OrbOfTransmutation, Currency::OrbOfAlteration];
        for variant in variants {
            match variant {
                Currency::OrbOfTransmutation | Currency::OrbOfAlteration => {}
            }
        }
        variants.to_vec()
    }

    /// The reasons with which the currency can fail (ADR 0012). Each has a game message.
    fn failure_reasons(currency: Currency) -> Vec<CraftError> {
        match currency {
            Currency::OrbOfTransmutation => vec![CraftError::ItemNotNormal],
            Currency::OrbOfAlteration => vec![CraftError::ItemNotMagic],
        }
    }

    #[test]
    fn every_currency_has_a_data_entry_with_a_name() {
        let entries = currencies();
        for currency in all() {
            let entry = entries
                .iter()
                .find(|entry| entry.id == currency.id())
                .unwrap_or_else(|| panic!("no data entry for {currency:?}"));
            assert!(!entry.name.is_empty());
            assert_eq!(currency.name(), entry.name);
        }
    }

    #[test]
    fn every_currency_has_a_description_and_a_message_for_every_reason() {
        let entries = currencies();
        for currency in all() {
            let entry = entries
                .iter()
                .find(|entry| entry.id == currency.id())
                .unwrap_or_else(|| panic!("no data entry for {currency:?}"));
            assert!(!entry.description.is_empty());
            assert_eq!(currency.description(), entry.description);
            let reasons = failure_reasons(currency);
            for error in &reasons {
                let message = currency.failure_message(*error);
                assert!(!message.is_empty(), "{currency:?} {error:?}");
                assert_eq!(Some(&message), entry.messages.get(error.code()));
            }
            assert_eq!(entry.messages.len(), reasons.len(), "{currency:?}");
        }
    }

    #[test]
    fn currency_ids_are_unique() {
        let ids: Vec<&str> = all().into_iter().map(Currency::id).collect();
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
