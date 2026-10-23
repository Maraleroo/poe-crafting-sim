//! Currencies: the game data entries and the currencies the crafting rules implement.

use serde::Deserialize;

use crate::data::currencies;

/// A currency in the game data (ADR 0011).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CurrencyDefinition {
    /// Project identifier, e.g. `orb_of_transmutation`. Not a game value.
    pub id: String,
    /// English game text, passed through unchanged.
    pub name: String,
}

/// A currency with crafting rules in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    OrbOfTransmutation,
}

impl Currency {
    /// The identifier of the currency's entry in the game data.
    pub fn id(self) -> &'static str {
        match self {
            Currency::OrbOfTransmutation => "orb_of_transmutation",
        }
    }

    /// The name of the currency from the game data.
    ///
    /// # Panics
    /// If the game data has no entry for the currency. That is a programming error, caught
    /// by the unit tests.
    pub fn name(self) -> String {
        let id = self.id();
        currencies()
            .into_iter()
            .find(|currency| currency.id == id)
            .unwrap_or_else(|| panic!("the game data has no currency {id:?}"))
            .name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant of [`Currency`]. The exhaustive match makes a new variant a compile
    /// error here until it is added to the list.
    fn all() -> Vec<Currency> {
        let variants = [Currency::OrbOfTransmutation];
        for variant in variants {
            match variant {
                Currency::OrbOfTransmutation => {}
            }
        }
        variants.to_vec()
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
    fn currency_ids_are_unique() {
        let ids: Vec<&str> = all().into_iter().map(Currency::id).collect();
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
