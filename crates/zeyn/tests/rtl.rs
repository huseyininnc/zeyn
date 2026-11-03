//! Right to left: the framework stylesheets lay out by reading direction. Every declaration that
//! names a physical side (`margin-left`, `left:`, `text-align: right`, a four value `padding` with
//! different sides, an inset box shadow or `translateX` with a fixed sign) fails this test unless
//! it is in [`ALLOWED`] with the reason it stays physical. Use logical properties instead
//! (`margin-inline-start`, `inset-inline-end`, `text-align: end`), or multiply the offset by
//! `var(--z-dir)`, which is 1 left to right and -1 right to left.

use std::path::Path;

/// One declaration that stays physical on purpose: file, selector, property and why.
struct Allowed {
    file: &'static str,
    selector: &'static str,
    property: &'static str,
    reason: &'static str,
}

const GLYPH: &str =
    "chevron drawn with two borders and rotated; [dir=\"rtl\"] mirrors the whole glyph with `scale: -1 1`";
const VERTICAL_GLYPH: &str =
    "arrow or check mark that points down, up or is a check; it does not follow reading direction";

/// Declarations that keep a physical side, each with its reason.
const ALLOWED: &[Allowed] = &[
    Allowed { file: "components.css", selector: ".z-chevron", property: "border-right", reason: GLYPH },
    Allowed {
        file: "components.css",
        selector: "[aria-expanded] > .z-tree-row > .z-tree-toggle::before",
        property: "border-right",
        reason: GLYPH,
    },
    Allowed {
        file: "components.css",
        selector: ".z-breadcrumb li + li::before",
        property: "border-right",
        reason: GLYPH,
    },
    Allowed { file: "pro.css", selector: ".z-form-section-title::before", property: "border-right", reason: GLYPH },
    Allowed { file: "content.css", selector: ".z-spoiler-label::before", property: "border-right", reason: GLYPH },
    Allowed { file: "zeyn.css", selector: ".z-expander::before", property: "border-right", reason: GLYPH },
    Allowed {
        file: "zeyn.css",
        selector: ".z-menu-item[aria-haspopup=\"menu\"]::after",
        property: "border-right",
        reason: GLYPH,
    },
    Allowed { file: "components.css", selector: ".z-select-arrow", property: "border-right", reason: VERTICAL_GLYPH },
    Allowed {
        file: "components.css",
        selector: ".z-option[aria-selected=\"true\"]:not(:has(.z-option-check))::after",
        property: "border-right",
        reason: VERTICAL_GLYPH,
    },
    Allowed {
        file: "components.css",
        selector: ".z-field input.z-checkbox:checked::after",
        property: "border-right",
        reason: VERTICAL_GLYPH,
    },
    Allowed { file: "site.css", selector: ".z-site-caret", property: "border-right", reason: VERTICAL_GLYPH },
    Allowed {
        file: "components.css",
        selector: ".z-pop",
        property: "left",
        reason: "fixed popup placed by the runtime in viewport pixels, which are physical; the runtime anchors it to the start side",
    },
    Allowed { file: "components.css", selector: ".z-split-v .z-split-bar::after", property: "inset", reason: CENTERED },
    Allowed {
        file: "components.css",
        selector: ".z-split-v .z-split-bar::after",
        property: "transform",
        reason: CENTERED,
    },
    Allowed {
        file: "media.css",
        selector: ".z-player-video:not(.z-playing) .z-player-stage::after",
        property: "inset",
        reason: CENTERED,
    },
    Allowed { file: "media.css", selector: ".z-embed-play", property: "inset", reason: CENTERED },
];

const CENTERED: &str = "centered with left 50% and a -50% translate, the same in both directions";

/// The stylesheets of the framework, read from `assets/`, so a new file is checked too.
fn stylesheets() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "css"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

/// Removes `/* ... */` comments.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at + 2..].find("*/").map_or("", |end| &rest[at + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// Every declaration with the selector of its rule: `(selector, property, value)`.
fn declarations(css: &str) -> Vec<(String, String, String)> {
    let css = strip_comments(css);
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    for c in css.chars() {
        if let Some(q) = quote {
            buf.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                buf.push(c);
            }
            '{' => {
                stack.push(buf.split_whitespace().collect::<Vec<_>>().join(" "));
                buf.clear();
            }
            ';' | '}' => {
                if let (Some(sel), Some((p, v))) = (stack.last(), buf.split_once(':')) {
                    out.push((sel.clone(), p.trim().to_string(), v.trim().to_string()));
                }
                buf.clear();
                if c == '}' {
                    stack.pop();
                }
            }
            _ => buf.push(c),
        }
    }
    out
}

/// Splits a value on spaces or commas that are not inside parentheses.
fn split_top(value: &str, sep: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0;
    for c in value.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && (c == sep || (sep == ' ' && c.is_whitespace())) {
            parts.push(String::new());
        } else {
            parts.last_mut().unwrap().push(c);
        }
    }
    parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// Why a declaration names a physical side, or `None` when it does not.
fn physical(property: &str, value: &str) -> Option<&'static str> {
    let side = property.ends_with("-left")
        || property.ends_with("-right")
        || property.contains("-left-")
        || property.contains("-right-");
    if property == "left" || property == "right" {
        return Some("use inset-inline-start or inset-inline-end");
    }
    if side
        && (property.starts_with("margin")
            || property.starts_with("padding")
            || property.starts_with("border")
            || property.starts_with("scroll"))
    {
        return Some("use the -inline-start or -inline-end form");
    }
    if matches!(property, "text-align" | "float" | "clear" | "justify-self" | "justify-items")
        && matches!(value, "left" | "right")
    {
        return Some("use start or end");
    }
    if matches!(property, "margin" | "padding" | "inset" | "border-width" | "border-color" | "border-style") {
        let v = split_top(value, ' ');
        if v.len() == 4 && v[1] != v[3] {
            return Some("the two inline sides differ; use the -block and -inline forms (padding-inline: start end)");
        }
    }
    if property == "border-radius" && !value.contains('/') {
        let v = split_top(value, ' ');
        let mirrored = match v.len() {
            2 => v[0] == v[1],
            3 => v[0] == v[1] && v[1] == v[2],
            4 => v[0] == v[1] && v[2] == v[3],
            _ => true,
        };
        if !mirrored {
            return Some("use border-start-start-radius and the other logical corners");
        }
    }
    if property == "box-shadow" {
        let fixed = split_top(value, ',').iter().any(|shadow| {
            let v = split_top(shadow, ' ');
            v.first().is_some_and(|f| f == "inset") && v.get(1).is_some_and(|x| x != "0" && !x.contains("var(--z-dir)"))
        });
        if fixed {
            return Some(
                "an inset shadow with a horizontal offset marks one side; multiply the offset by var(--z-dir)",
            );
        }
    }
    if property == "transform" && value.contains("translateX(") && !value.contains("var(--z-dir)") {
        return Some("a horizontal slide has a side; multiply it by var(--z-dir)");
    }
    None
}

#[test]
fn framework_css_uses_logical_properties_for_reading_direction() {
    let mut found = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for (file, css) in stylesheets() {
        for (selector, property, value) in declarations(&css) {
            let Some(why) = physical(&property, &value) else { continue };
            let allowed =
                ALLOWED.iter().position(|a| a.file == file && a.selector == selector && a.property == property);
            match allowed {
                Some(i) => used[i] = true,
                None => found.push(format!("{file}: `{selector} {{ {property}: {value} }}`: {why}")),
            }
        }
    }
    assert!(
        found.is_empty(),
        "physical direction in the framework CSS breaks right to left pages. Fix each one, or add it \
         to ALLOWED in tests/rtl.rs with the reason it stays physical:\n{}",
        found.join("\n")
    );
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(a, _)| format!("{}: {} {} ({})", a.file, a.selector, a.property, a.reason))
        .collect();
    assert!(stale.is_empty(), "ALLOWED lists declarations that are gone; remove them:\n{}", stale.join("\n"));
}

#[test]
fn the_checker_catches_physical_sides_and_lets_logical_ones_pass() {
    let css = ".a { margin-left: 2px; padding: 0 1px 0 2px; box-shadow: inset 2px 0 0 red; } \
               .b { margin-inline-start: 2px; padding: 0 1px; box-shadow: inset calc(2px * var(--z-dir)) 0 0 red, inset 0 0 0 1px red; } \
               @media (max-width: 1px) { .c { text-align: right; } }";
    let flagged: Vec<_> = declarations(css)
        .into_iter()
        .filter(|(_, p, v)| physical(p, v).is_some())
        .map(|(s, p, _)| format!("{s} {p}"))
        .collect();
    assert_eq!(flagged, [".a margin-left", ".a padding", ".a box-shadow", ".c text-align"]);
}
