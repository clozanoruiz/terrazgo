// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A record brought back, compared with what it was before it went.
//!
//! Every register's crate pins that deleting a book takes its records and that
//! bringing the book back restores them whole (docs/sync.md → Deleting a book
//! with its records), and each compares the record as its own `get_*` returns
//! it. One comparison, so each crate means the same by "as it was".

use serde_json::Value;

/// Assert that a record brought back reads as it did before it went — every
/// field of it and of every row belonging to it — but for `updated_at`, which
/// a write stamps. `before` and `after` are the record as a repository's
/// `get_*` returns it, serialised with `serde_json::to_value`.
pub fn assert_restored(before: &Value, after: &Value) {
    assert_eq!(
        unstamped(before),
        unstamped(after),
        "a record brought back reads as it did before it went"
    );
}

/// `value` with every `updated_at` taken out, at any depth.
fn unstamped(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "updated_at")
                .map(|(key, value)| (key.clone(), unstamped(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(unstamped).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_stamp_is_ignored_at_any_depth() {
        assert_restored(
            &json!({ "id": "r", "updated_at": "a", "plots": [{ "id": "p", "updated_at": "a" }] }),
            &json!({ "id": "r", "updated_at": "b", "plots": [{ "id": "p", "updated_at": "b" }] }),
        );
    }

    #[test]
    #[should_panic(expected = "as it did before it went")]
    fn anything_else_that_differs_is_caught() {
        assert_restored(
            &json!({ "id": "r", "plots": [{ "id": "p" }, { "id": "q" }] }),
            &json!({ "id": "r", "plots": [{ "id": "p" }] }),
        );
    }

    #[test]
    #[should_panic(expected = "as it did before it went")]
    fn a_removal_left_in_place_is_caught() {
        assert_restored(
            &json!({ "id": "r", "deleted_at": null }),
            &json!({ "id": "r", "deleted_at": "2026-10-01T00:00:00Z" }),
        );
    }
}
