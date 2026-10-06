// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Form-feedback contract: a form says what is wrong in ONE place, in the
//! holding's language.
//!
//! Three rules, each banning a construct by name, in the shape
//! `number_formatting.rs` established — and for the same reason. What they
//! guard is invisible at the call site: a raw `<form>` still submits, a raw
//! `<input required>` still blocks, and a `required` attribute on a validity
//! proxy still refuses. Every one of them WORKS. What they do is report the
//! failure in the browser's own words, which follow the OPERATING SYSTEM's
//! language rather than the one the holding chose — measured 2026-09-01 in
//! headless Chrome, where a bare `required` answers "Please fill in this
//! field." That is the same defect that retired the native date picker, and it
//! had gone unnoticed at 23 call sites precisely because nothing failed.
//!
//! # What this can and cannot catch
//!
//! It catches the *class*: markup that opts out of the shared surface. It
//! cannot check that a form's `anchors` map names a field that exists, or that
//! the message reads well — those stay review work, which is why the rules are
//! also written down in `docs/frontend-conventions.md`.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// The frontend tree. Nothing in Rust renders a form.
const ROOT: &str = "src";

/// The one component allowed to render a `<form>`: it is the shared surface
/// every other form goes through.
const FORM_OWNER: &str = "TzForm.svelte";

/// The owned controls, which are allowed to render a real `<input>` — that is
/// what they are. Each drives its validity through `setCustomValidity` so the
/// message stays ours; the third rule below is what holds them to it.
const OWNED_CONTROLS: [&str; 4] = [
    "TextInput.svelte",
    "TzCheckbox.svelte",
    "NumberInput.svelte",
    "CataloguePicker.svelte",
];

/// Controls whose visible part is not a labelable element, so constraint
/// validation rides on an off-screen `.tz-validity` proxy instead.
const PROXY_CONTROLS: [&str; 5] = [
    "DateInput.svelte",
    "TimeInput.svelte",
    "TzSelect.svelte",
    "TzCombobox.svelte",
    "NumberInput.svelte",
];

const EXTENSIONS: &[&str] = &["js", "svelte"];
const SKIP_DIRS: &[&str] = &["node_modules", "dist"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .to_path_buf()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| SKIP_DIRS.iter().any(|d| n == *d))
            {
                continue;
            }
            collect_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| EXTENSIONS.iter().any(|e| ext == *e))
        {
            out.push(path);
        }
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

/// Every frontend file, with its text. Asserts the walk found something, so a
/// moved tree fails loudly instead of passing vacuously.
fn frontend_files() -> Vec<(PathBuf, String)> {
    let root = workspace_root();
    let mut paths = Vec::new();
    collect_files(&root.join(ROOT), &mut paths);
    assert!(
        paths.len() > 20,
        "expected to walk the frontend tree, found {} files — has {ROOT}/ moved?",
        paths.len()
    );
    paths
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).expect("frontend file is readable UTF-8");
            (p, text)
        })
        .collect()
}

/// Occurrences of `needle` that open a TAG rather than sit in prose — `<form`
/// in a comment sentence is documentation, `<form ` or `<form>` is markup.
fn tag_openings(text: &str, needle: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(needle) {
        let at = from + at;
        from = at + needle.len();
        let next = text[at + needle.len()..].chars().next();
        if matches!(next, Some(c) if c.is_whitespace() || c == '>') {
            found.push(at);
        }
    }
    found
}

#[test]
fn only_tz_form_renders_a_form_element() {
    let mut findings = Vec::new();
    for (path, text) in frontend_files() {
        if file_name(&path) == FORM_OWNER {
            continue;
        }
        for at in tag_openings(&text, "<form") {
            findings.push(format!(
                "{}:{} — a bare <form>. Use TzForm, which reports every problem \
                 at once instead of letting the browser paint one bubble in the \
                 OS language.",
                path.display(),
                line_of(&text, at),
            ));
        }
    }
    assert!(findings.is_empty(), "\n{}\n", findings.join("\n"));
}

#[test]
fn a_required_field_is_an_owned_control() {
    let mut findings = Vec::new();
    for (path, text) in frontend_files() {
        if OWNED_CONTROLS.contains(&file_name(&path)) {
            continue;
        }
        // A raw `<input …>` carrying `required` anywhere before its close.
        for at in tag_openings(&text, "<input") {
            let end = text[at..].find('>').map(|e| at + e).unwrap_or(text.len());
            if text[at..end].contains("required") {
                findings.push(format!(
                    "{}:{} — `required` on a raw <input>. Its message would be \
                     the browser's, in the OS language; use TextInput (or the \
                     owned control for that type).",
                    path.display(),
                    line_of(&text, at),
                ));
            }
        }
    }
    assert!(findings.is_empty(), "\n{}\n", findings.join("\n"));
}

#[test]
fn a_validity_proxy_never_carries_the_required_attribute() {
    let mut findings = Vec::new();
    for (path, text) in frontend_files() {
        if !PROXY_CONTROLS.contains(&file_name(&path)) {
            continue;
        }
        // The proxy is the input carrying class="tz-validity"; read to its
        // close and refuse a `required` attribute there. `{required}` is the
        // shorthand Svelte spells an attribute with, so both forms are caught.
        let Some(at) = text.find("class=\"tz-validity\"") else {
            panic!(
                "{}: no .tz-validity proxy found — has the control changed shape? \
                 This test would then be checking nothing.",
                path.display()
            );
        };
        let end = text[at..].find("/>").map(|e| at + e).unwrap_or(text.len());
        let proxy = &text[at..end];
        if proxy.contains("required") {
            findings.push(format!(
                "{}:{} — the validity proxy carries `required`. That leaves \
                 validationMessage as the BROWSER's string, so TzForm's summary \
                 would read it in the OS language. Drive it with \
                 setCustomValidity(t(\"form.required\")) instead.",
                path.display(),
                line_of(&text, at),
            ));
        }
    }
    assert!(findings.is_empty(), "\n{}\n", findings.join("\n"));
}
