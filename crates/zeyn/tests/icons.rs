//! The icon set: every `Icon` has a drawing in `assets/icons/`, every drawing an `Icon`, the
//! drawings follow the grid and stroke of the set, and `assets/icons.css` is generated from them.
//!
//! After adding or changing a drawing, write the stylesheet again with
//! `ZEYN_WRITE_ICONS=1 cargo test -p zeyn --test icons`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use zeyn::prelude::*;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// The drawings by name, read from `assets/icons/*.svg`.
fn drawings() -> BTreeMap<String, String> {
    std::fs::read_dir(dir().join("icons"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e == "svg")
        })
        .map(|p| {
            let name = p
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

/// The stylesheet for the drawings: one mask per icon, in the order of `Icon::ALL`.
fn stylesheet(drawings: &BTreeMap<String, String>) -> String {
    let mut css =
        String::from("/* Generated from assets/icons/*.svg by tests/icons.rs. Do not edit. */\n");
    for icon in Icon::ALL {
        let svg = drawings[icon.name()]
            .trim()
            .replace('"', "'")
            .replace('<', "%3C")
            .replace('>', "%3E")
            .replace('#', "%23");
        css.push_str(&format!(
            ".z-icon-{}{{--z-i:url(\"data:image/svg+xml,{svg}\")}}\n",
            icon.name()
        ));
    }
    css
}

#[test]
fn every_icon_has_a_drawing_and_every_drawing_an_icon() {
    let drawings = drawings();
    for icon in Icon::ALL {
        assert!(
            drawings.contains_key(icon.name()),
            "Icon::{icon:?} has no assets/icons/{}.svg",
            icon.name()
        );
    }
    for name in drawings.keys() {
        assert!(
            Icon::ALL
                .iter()
                .any(|i| i.name() == name),
            "assets/icons/{name}.svg has no Icon variant"
        );
    }
    assert_eq!(Icon::ALL.len(), drawings.len());
}

#[test]
fn drawings_use_the_grid_and_stroke_of_the_set() {
    for (name, svg) in drawings() {
        assert!(
            svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="black" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#),
            "{name}: draw on the 20 px grid with a 1.5 px round stroke and no fill"
        );
        for width in svg
            .match_indices("stroke-width=\"")
            .map(|(i, m)| &svg[i + m.len()..i + m.len() + 3])
        {
            assert!(
                width == "1.5" || width == "2.5",
                "{name}: stroke width {width}; lines are 1.5, dots 2.5"
            );
        }
        for forbidden in ["fill=\"black\"", "<image", "<script", "href", "style="] {
            assert!(!svg.contains(forbidden), "{name}: no {forbidden}");
        }
        assert!(svg.len() < 600, "{name}: {} bytes", svg.len());
    }
}

#[test]
fn the_stylesheet_is_generated_from_the_drawings() {
    let css = stylesheet(&drawings());
    let path = dir().join("icons.css");
    if std::env::var_os("ZEYN_WRITE_ICONS").is_some() {
        std::fs::write(&path, &css).unwrap();
    }
    let written = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        written == css,
        "assets/icons.css is out of date; run `ZEYN_WRITE_ICONS=1 cargo test -p zeyn --test icons`"
    );
}

#[test]
fn icon_markup_uses_the_class_of_its_file() {
    assert_eq!(
        Icon::ChartBar.html(),
        r#"<span class="z-icon z-icon-chart-bar" aria-hidden="true"></span>"#
    );
    assert_eq!(
        serde_json::to_value(Icon::SignOut).unwrap(),
        "sign-out",
        "menus built in the browser use the serialized name as the class"
    );
}
