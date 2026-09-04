// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Ordering the names the book prints, in the book's own language.
//!
//! Every list of names in this crate used to be sorted with Rust's derived
//! `Ord`, which compares UTF-8 bytes. That is wrong in two visible ways for the
//! languages this book prints in:
//!
//! * `Á` is U+00C1 and `Z` is U+005A, so a plot called *Ángel* printed **after**
//!   *Zubiri*, and *Muñoz* after *Muzquiz*. Spanish and Catalan names carry
//!   accents constantly, so this was an everyday defect rather than an edge case.
//! * A byte sort compares digits as characters, so *Parcela 10* printed before
//!   *Parcela 2* — which Spanish plot naming hits even more often than accents.
//!
//! Both are display ordering inside a legal document, never a legal assertion:
//! the same rows print either way, in a different order. That is why a collator
//! that cannot be built degrades to the old byte ordering instead of refusing to
//! print a record book (see [`NameCollator::new`]).
//!
//! The frontend does the same job with `Intl.Collator` over the same CLDR data
//! and the same options (`src/lib/collate.js`), so the RULES are the same on
//! both sides. Which language's rules apply is not: this book uses its own
//! report language and the screen uses the one being read. They coincide
//! whenever those match — the common case — and genuinely differ otherwise,
//! since Castilian files `ñ` as a letter after `n` (so *Peña* after *Penz*)
//! where Catalan and English fold it beside `n`.

use std::cmp::Ordering;

use icu_collator::{
    Collator, CollatorBorrowed, CollatorPreferences,
    options::{CollatorOptions, Strength},
    preferences::CollationNumericOrdering,
};
use icu_locale_core::locale;

use crate::labels::ReportLanguage;

/// Compares names the way the book's language does.
///
/// Built once per rendered book and shared, because constructing a collator
/// does a data lookup that has no business running once per printed row.
pub(crate) struct NameCollator {
    /// `None` only if the compiled collation data could not be loaded, which
    /// would be a broken build rather than a runtime condition — a test asserts
    /// it is `Some` for every language the book can print.
    inner: Option<CollatorBorrowed<'static>>,
}

impl NameCollator {
    pub(crate) fn new(language: ReportLanguage) -> Self {
        // A closed match rather than parsing `language.code()`: the set of
        // languages is fixed at compile time, so this needs no error path.
        let mut prefs: CollatorPreferences = match language {
            ReportLanguage::Es => locale!("es").into(),
            ReportLanguage::Ca => locale!("ca").into(),
        };
        // "Parcela 2" before "Parcela 10": digits compare by value, not by
        // character. This is the BCP-47 `-u-kn` key.
        prefs.numeric_ordering = Some(CollationNumericOrdering::True);

        let mut options = CollatorOptions::default();
        // Tertiary: base letters first, then accents, then case — so `Pena`
        // still precedes `Peña` rather than tying with it. Primary strength
        // would fold the accent away entirely and leave the order arbitrary.
        options.strength = Some(Strength::Tertiary);

        Self {
            inner: Collator::try_new(prefs, options).ok(),
        }
    }

    pub(crate) fn compare(&self, a: &str, b: &str) -> Ordering {
        match &self.inner {
            Some(collator) => collator.compare(a, b),
            // The documented degradation: byte order, which is what this code
            // did before the collator existed.
            None => a.cmp(b),
        }
    }

    /// Sorts in place. Stable, unlike the `sort_unstable` calls this replaced:
    /// a collator can report two DIFFERENT strings as `Equal` (same letters,
    /// different accents at a lower strength), and their input order is then
    /// the only thing left to keep the output reproducible.
    pub(crate) fn sort(&self, names: &mut [&str]) {
        names.sort_by(|a, b| self.compare(a, b));
    }

    /// The owned-string variant, for the callers that build `Vec<String>`.
    pub(crate) fn sort_owned(&self, names: &mut [String]) {
        names.sort_by(|a, b| self.compare(a, b));
    }
}

impl Default for NameCollator {
    /// Castilian, the book's default language — so a `PlotIndex` built by
    /// `Default` still orders sensibly rather than falling back to bytes.
    fn default() -> Self {
        Self::new(ReportLanguage::Es)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled collation data must actually be there for every language
    /// the book prints. Without this, a packaging change could silently drop
    /// back to byte ordering and every other test here would still pass.
    #[test]
    fn every_report_language_has_collation_data() {
        for language in ReportLanguage::ALL {
            assert!(
                NameCollator::new(language).inner.is_some(),
                "no compiled collation data for {}",
                language.code()
            );
        }
    }

    /// The defect this module exists for: `Á` is U+00C1 and `Z` is U+005A, so a
    /// byte sort put every accented name last.
    #[test]
    fn accents_sort_by_base_letter_not_by_code_point() {
        let collator = NameCollator::new(ReportLanguage::Es);
        let mut names = vec!["Zubiri", "Ángel", "Bravo"];
        collator.sort(&mut names);
        assert_eq!(names, ["Ángel", "Bravo", "Zubiri"]);

        // And the byte order these replace really is wrong, so the test is
        // pinning a fix rather than restating the default.
        let mut bytes = vec!["Zubiri", "Ángel", "Bravo"];
        bytes.sort();
        assert_eq!(bytes, ["Bravo", "Zubiri", "Ángel"]);
    }

    /// RAE: `ñ` is a letter of its own, ordered after `n`. At tertiary strength
    /// the accent is a lower-level difference, so `Pena` precedes `Peña` —
    /// which is also what the byte order happened to give, but for the wrong
    /// reason, and the pair is here so a strength change cannot silently tie
    /// them.
    #[test]
    fn n_tilde_orders_after_plain_n() {
        let collator = NameCollator::new(ReportLanguage::Es);
        assert_eq!(collator.compare("Pena", "Peña"), Ordering::Less);
        assert_ne!(collator.compare("Pena", "Peña"), Ordering::Equal);
        assert_eq!(collator.compare("Muñoz", "Muzquiz"), Ordering::Less);
    }

    /// The one farmers hit most: plots are named "Parcela 1", "Parcela 2"…
    /// and a byte sort reads the digits as characters.
    #[test]
    fn digit_runs_compare_by_numeric_value() {
        let collator = NameCollator::new(ReportLanguage::Es);
        let mut names = vec!["Parcela 10", "Parcela 2", "Parcela 1"];
        collator.sort(&mut names);
        assert_eq!(names, ["Parcela 1", "Parcela 2", "Parcela 10"]);

        let mut bytes = vec!["Parcela 10", "Parcela 2", "Parcela 1"];
        bytes.sort();
        assert_eq!(bytes, ["Parcela 1", "Parcela 10", "Parcela 2"]);
    }

    /// The book prints in Catalan too, and its collator must be a real one
    /// rather than the Castilian instance under another name.
    #[test]
    fn catalan_orders_accented_names_too() {
        let collator = NameCollator::new(ReportLanguage::Ca);
        let mut names = vec!["Vinyes", "Òdena", "Camp"];
        collator.sort(&mut names);
        assert_eq!(names, ["Camp", "Òdena", "Vinyes"]);
    }

    /// Sorting must not depend on the input order of rows a collator calls
    /// equal — the reason these became `sort_by` rather than `sort_unstable_by`.
    #[test]
    fn sorting_is_stable_and_idempotent() {
        let collator = NameCollator::new(ReportLanguage::Es);
        let mut once = vec!["Ángel", "Bravo", "Ángel", "Zubiri"];
        collator.sort(&mut once);
        let mut twice = once.clone();
        collator.sort(&mut twice);
        assert_eq!(once, twice);
    }

    #[test]
    fn owned_and_borrowed_sorts_agree() {
        let collator = NameCollator::new(ReportLanguage::Es);
        let mut borrowed = vec!["Zubiri", "Ángel", "Parcela 10", "Parcela 2"];
        let mut owned: Vec<String> = borrowed.iter().map(|s| s.to_string()).collect();
        collator.sort(&mut borrowed);
        collator.sort_owned(&mut owned);
        assert_eq!(
            borrowed,
            owned.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }
}
