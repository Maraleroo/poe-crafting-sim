//! Which page the visitor asked for.
//!
//! The site is a single page. The page is chosen by the `page` query parameter, for
//! example `?page=privacy`. The links are relative, so they work under the Pages
//! sub-path and under `trunk serve`. Following a link reloads the page.

/// A page of the site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    PrivacyEnglish,
    PrivacyGerman,
}

impl Route {
    /// Route for the query string of the current location, for example `?page=privacy`.
    ///
    /// Anything that is not a known page, including a missing query, is the home page.
    pub fn from_search(search: &str) -> Self {
        let query = search.strip_prefix('?').unwrap_or(search);
        let page = query
            .split('&')
            .find_map(|parameter| parameter.strip_prefix("page="));
        match page {
            Some("privacy") => Self::PrivacyEnglish,
            Some("privacy-de") => Self::PrivacyGerman,
            _ => Self::Home,
        }
    }

    /// Relative link to the page, or `None` for the home page, which no link points to.
    pub fn href(self) -> Option<&'static str> {
        match self {
            Self::Home => None,
            Self::PrivacyEnglish => Some("?page=privacy"),
            Self::PrivacyGerman => Some("?page=privacy-de"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_pages_are_selected_by_the_page_parameter() {
        assert_eq!(Route::from_search("?page=privacy"), Route::PrivacyEnglish);
        assert_eq!(Route::from_search("?page=privacy-de"), Route::PrivacyGerman);
    }

    #[test]
    fn missing_or_unknown_pages_are_the_home_page() {
        for search in [
            "",
            "?",
            "?page=",
            "?page=unknown",
            "?page=privacy-fr",
            "?page=Privacy",
            "?other=privacy",
            "?xpage=privacy",
        ] {
            assert_eq!(Route::from_search(search), Route::Home, "{search:?}");
        }
    }

    #[test]
    fn the_page_parameter_may_have_company() {
        assert_eq!(
            Route::from_search("?lang=en&page=privacy-de&x=1"),
            Route::PrivacyGerman
        );
    }

    #[test]
    fn every_link_leads_to_its_own_route() {
        for route in [Route::PrivacyEnglish, Route::PrivacyGerman] {
            let href = route.href().expect("a legal page has a link");
            assert_eq!(Route::from_search(href), route);
        }
        assert_eq!(Route::Home.href(), None);
    }
}
