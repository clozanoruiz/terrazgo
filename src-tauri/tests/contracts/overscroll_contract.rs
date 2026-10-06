// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Overscroll contract: a box opts out of overscroll on the axis it scrolls,
//! and on no other.
//!
//! `overscroll-behavior: none` does two things. It suppresses the bounce (the
//! Android 12+ stretch, the glow before it), and it stops scroll chaining. A
//! touch scroll latches to the innermost scroll container under the finger, and
//! a container that says `none` on the swiped axis ends the gesture there — even
//! when it has nothing to scroll. Android System WebView 151 treats EVERY scroll
//! container that way: overflowing or not, `overflow: hidden` included.
//!
//! Measured on a phone (2026-09-13, `adb shell input swipe`): a universal
//! `* { overscroll-behavior: none }` turned every table cell (`overflow: hidden`
//! for the ellipsis), every `.table-wrap` and the map's side panel into dead
//! zones. A vertical swipe starting on one moved nothing, and a sideways swipe
//! on a table did not scroll the table. Putting `auto` back revived all of
//! them; putting it back on the wrappers alone revived none, because the cells
//! trap the gesture on their own.
//!
//! So the rule is checked per declaration block, statically, in every
//! stylesheet the app ships — `src/styles.css` and each component's `<style>`:
//!
//! * a block that opts out on an axis must itself declare that axis `auto` or
//!   `scroll`. That refuses the universal selector, a clipping box that opts
//!   out, and a sideways scroller that opts out vertically too;
//! * a block that declares an axis `auto` or `scroll` must opt out on it, so no
//!   real scroller is left to stretch its content at the ends;
//! * the opt-out is `none`, because `contain` stops the chaining and still shows
//!   the local bounce.
//!
//! A box that scrolls in one layout only (the map's side panel beside the map,
//! ordinary content under it on a phone) states its `overflow` inside that
//! layout's media query, where this contract reads it like any other block.
//!
//! `html` and `body` are the one exemption: nothing sits above them to chain to,
//! and their opt-out governs the webview surface itself.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
        }
    }
}

/// One `selector { declarations }` block. `line` is where its `{` sits.
#[derive(Debug)]
struct Block {
    selector: String,
    line: usize,
    declarations: Vec<(String, String)>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .to_path_buf()
}

/// Drop `/* … */` comments but keep their newlines, so line numbers survive.
/// The stylesheet's comments quote rules verbatim, braces and all.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    let mut in_comment = false;
    while let Some(c) = chars.next() {
        if in_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
            } else if c == '\n' {
                out.push('\n');
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// `prop: value` pairs of one block's own text, lowercased, `!important` shed.
fn declarations(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter_map(|declaration| {
            let (prop, value) = declaration.split_once(':')?;
            let prop = prop.trim().to_ascii_lowercase();
            if prop.is_empty() || prop.contains(char::is_whitespace) {
                return None;
            }
            let value = value.trim().to_ascii_lowercase();
            let value = value.trim_end_matches("!important").trim().to_string();
            Some((prop, value))
        })
        .collect()
}

/// Every block of a stylesheet, nested ones included (`@media`, nesting).
///
/// A frame collects the text of its own block. On `{`, whatever that text holds
/// since its last `;` is the child's selector, and it is cut out of the parent;
/// on `}` the child is complete. So a parent's declarations never include a
/// child's, and an at-rule wrapper comes out as a block with none.
fn blocks(css: &str, first_line: usize) -> Vec<Block> {
    struct Frame {
        selector: String,
        line: usize,
        own: String,
    }

    let css = strip_comments(css);
    let mut line = first_line;
    let mut stack = vec![Frame {
        selector: String::new(),
        line,
        own: String::new(),
    }];
    let mut out = Vec::new();
    for c in css.chars() {
        match c {
            '{' => {
                let parent = stack.last_mut().expect("the root frame is never popped");
                let cut = parent.own.rfind(';').map_or(0, |i| i + 1);
                let selector = parent.own[cut..].trim().to_string();
                parent.own.truncate(cut);
                stack.push(Frame {
                    selector,
                    line,
                    own: String::new(),
                });
            }
            '}' if stack.len() > 1 => {
                let frame = stack.pop().expect("checked by the guard");
                out.push(Block {
                    selector: frame.selector,
                    line: frame.line,
                    declarations: declarations(&frame.own),
                });
                // A closed child ends a statement in its parent, so the next
                // selector does not start with this one's leftovers.
                stack.last_mut().expect("root frame").own.push(';');
            }
            _ => {
                if c == '\n' {
                    line += 1;
                }
                stack.last_mut().expect("root frame").own.push(c);
            }
        }
    }
    out
}

/// The value each axis ends the block with, for a property whose shorthand
/// takes `<x> [<y>]` — `overflow` and `overscroll-behavior` both do. Logical
/// longhands map to physical axes: every locale the app ships is horizontal.
fn per_axis(block: &Block, shorthand: &str) -> BTreeMap<Axis, String> {
    let mut axes = BTreeMap::new();
    for (prop, value) in &block.declarations {
        if prop == shorthand {
            let mut parts = value.split_whitespace();
            let x = parts.next().unwrap_or_default().to_string();
            let y = parts.next().map_or_else(|| x.clone(), str::to_string);
            axes.insert(Axis::X, x);
            axes.insert(Axis::Y, y);
        } else if let Some(suffix) = prop
            .strip_prefix(shorthand)
            .and_then(|rest| rest.strip_prefix('-'))
        {
            // `overflow-wrap` and `overflow-clip-margin` share the prefix and
            // name no axis.
            let axis = match suffix {
                "x" | "inline" => Axis::X,
                "y" | "block" => Axis::Y,
                _ => continue,
            };
            axes.insert(axis, value.clone());
        }
    }
    axes
}

fn is_root(selector: &str) -> bool {
    selector
        .split(',')
        .all(|part| matches!(part.trim(), "html" | "body" | ":root"))
}

/// What is wrong with one block, one line per broken axis.
fn violations(block: &Block) -> Vec<String> {
    let scrolls: BTreeSet<Axis> = per_axis(block, "overflow")
        .into_iter()
        .filter(|(_, value)| value == "auto" || value == "scroll")
        .map(|(axis, _)| axis)
        .collect();
    let opt_outs = per_axis(block, "overscroll-behavior");

    let mut found = Vec::new();
    for (axis, value) in &opt_outs {
        if value == "auto" {
            continue;
        }
        if value != "none" {
            found.push(format!(
                "overscroll-behavior on {} is `{value}`; the opt-out is `none` \
                 (`contain` stops the chaining and still bounces)",
                axis.name()
            ));
        }
        if !scrolls.contains(axis) && !is_root(&block.selector) {
            found.push(format!(
                "opts out on {} without scrolling on it, so a swipe that starts \
                 here can never reach the scroller above",
                axis.name()
            ));
        }
    }
    for axis in &scrolls {
        if opt_outs.get(axis).is_none_or(|value| value == "auto") {
            found.push(format!(
                "scrolls on {0} without `overscroll-behavior-{0}: none`, so it \
                 stretches its content at either end",
                axis.name()
            ));
        }
    }
    found
}

/// Each `<style>` element of a component, with the line its content starts on.
fn style_elements(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = source[from..].find("<style").map(|i| from + i) {
        let Some(content_start) = source[open..].find('>').map(|i| open + i + 1) else {
            break;
        };
        let content_end = source[content_start..]
            .find("</style>")
            .map_or(source.len(), |i| content_start + i);
        let line = source[..content_start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        out.push((line, &source[content_start..content_end]));
        from = content_end;
    }
    out
}

fn collect_stylesheet_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("src/ is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            collect_stylesheet_sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "css" || ext == "svelte")
        {
            out.push(path);
        }
    }
}

/// Violations across one file, as `path:line selector: problem`.
fn file_violations(rel: &str, source: &str, is_component: bool) -> (usize, Vec<String>) {
    let sheets = if is_component {
        style_elements(source)
    } else {
        vec![(1, source)]
    };
    let mut count = 0;
    let mut found = Vec::new();
    for (first_line, css) in sheets {
        for block in blocks(css, first_line) {
            count += 1;
            for problem in violations(&block) {
                found.push(format!(
                    "{rel}:{} `{}`: {problem}",
                    block.line, block.selector
                ));
            }
        }
    }
    (count, found)
}

#[test]
fn every_stylesheet_opts_out_only_where_it_scrolls() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_stylesheet_sources(&root.join("src"), &mut files);
    files.sort();

    let mut block_count = 0;
    let mut hits = Vec::new();
    for path in &files {
        let source =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let is_component = path.extension().is_some_and(|ext| ext == "svelte");
        let (count, found) = file_violations(&rel, &source, is_component);
        block_count += count;
        hits.extend(found);
    }

    // A walker that silently found nothing would pass everything.
    assert!(
        files.iter().any(|p| p.ends_with("src/styles.css")),
        "src/styles.css not found — did the stylesheet move?"
    );
    assert!(
        block_count > 300,
        "only {block_count} CSS blocks read — is the parser still seeing the sheets?"
    );
    assert!(
        hits.is_empty(),
        "a box opts out of overscroll on the axis it scrolls and on no other \
         (see this file's header):\n{}",
        hits.join("\n")
    );
}

// --- the checker itself, on samples -------------------------------------------

fn sample(css: &str) -> Vec<String> {
    blocks(css, 1).iter().flat_map(violations).collect()
}

#[test]
fn the_universal_opt_out_is_refused_on_both_axes() {
    let found = sample("* { overscroll-behavior: none; }");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|f| f.contains("without scrolling")));
}

#[test]
fn a_sideways_scroller_may_not_opt_out_vertically() {
    let found = sample(".table-wrap { overflow-x: auto; overscroll-behavior: none; }");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("opts out on y"));

    assert!(sample(".table-wrap { overflow-x: auto; overscroll-behavior-x: none; }").is_empty());
}

#[test]
fn a_clipping_box_leaves_overscroll_alone() {
    assert!(sample("td { overflow: hidden; text-overflow: ellipsis; }").is_empty());
    assert_eq!(
        sample("td { overflow: hidden; overscroll-behavior: none; }").len(),
        2
    );
}

#[test]
fn a_scroller_that_does_not_opt_out_is_refused() {
    let found = sample(".list { max-height: 16rem; overflow-y: auto; }");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("scrolls on y"));

    let found = sample(".lic-text { overflow: auto; overscroll-behavior-y: none; }");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("scrolls on x"));
}

#[test]
fn contain_is_not_an_opt_out() {
    let found = sample(".list { overflow-y: auto; overscroll-behavior-y: contain; }");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`contain`"));
}

#[test]
fn the_root_may_opt_out_without_scrolling() {
    assert!(sample("html,\nbody { height: 100%; overscroll-behavior: none; }").is_empty());
    assert_eq!(
        sample("html, main { overscroll-behavior: none; }").len(),
        2,
        "the exemption covers a list of root elements only"
    );
}

#[test]
fn a_scroller_inside_a_media_query_is_read() {
    let css = "@media (min-width: 701px) {\n  .side { overflow-y: auto; }\n}";
    let parsed = blocks(css, 1);
    let side = parsed
        .iter()
        .find(|b| b.selector == ".side")
        .expect("nested block");
    assert_eq!(side.line, 2);
    assert_eq!(violations(side).len(), 1);

    let wrapper = parsed
        .iter()
        .find(|b| b.selector.starts_with("@media"))
        .expect("wrapper block");
    assert!(
        wrapper.declarations.is_empty(),
        "{:?}",
        wrapper.declarations
    );
}

#[test]
fn two_value_shorthands_split_by_axis() {
    assert!(sample(".box { overflow: hidden auto; overscroll-behavior: auto none; }").is_empty());
    let found = sample(".box { overflow: hidden auto; overscroll-behavior: none auto; }");
    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn later_declarations_win_within_a_block() {
    assert!(sample(".box { overflow-y: auto; overflow: visible; }").is_empty());
    assert_eq!(
        sample(".box { overscroll-behavior: none; overscroll-behavior: auto; overflow-y: auto; }")
            .len(),
        1
    );
}

#[test]
fn look_alike_properties_name_no_axis() {
    assert!(
        sample(".strip { overflow-x: clip; overflow-clip-margin: 3px; overflow-wrap: anywhere; }")
            .is_empty()
    );
}

#[test]
fn comments_are_not_rules_and_keep_line_numbers() {
    let css = "/* was: * { overscroll-behavior: none }\n   and more */\n.a { color: red; }\n.b {\n  overflow-y: auto;\n}";
    let parsed = blocks(css, 1);
    assert_eq!(
        parsed
            .iter()
            .map(|b| b.selector.as_str())
            .collect::<Vec<_>>(),
        [".a", ".b"]
    );
    assert_eq!(parsed[1].line, 4);
}

#[test]
fn a_component_style_element_is_read_at_its_own_lines() {
    let component = "<script>\n  let a = 1;\n</script>\n\n<div></div>\n\n<style>\n  .a {\n    overflow: auto;\n  }\n</style>\n";
    let (count, found) = file_violations("src/lib/X.svelte", component, true);
    assert_eq!(count, 1);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with("src/lib/X.svelte:8 `.a`"), "{found:?}");
}
