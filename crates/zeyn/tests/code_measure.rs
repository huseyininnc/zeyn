//! Time to render a `CodeView` of a 5,000 line Rust file on the server, and the size of its
//! HTML. Ignored by default; run in release with
//! `cargo test --release -p zeyn --test code_measure -- --ignored --nocapture`.

use std::time::Instant;
use zeyn::ui::{CodeLang, CodeView, JsonView};
use zeyn::{Component, I18n, Render};

fn source(lines: usize) -> String {
    let block = "/// A documented function.\npub fn total(items: &[Line]) -> Money {\n    let mut sum = 0i64;\n    for item in items {\n        if item.qty > 0 && item.name != \"skip\" {\n            sum += item.price * i64::from(item.qty); // running total\n        }\n    }\n    Money::tl(sum)\n}\n";
    block
        .lines()
        .cycle()
        .take(lines)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore]
fn a_five_thousand_line_file_renders_on_the_server() {
    let text = source(5_000);
    let i18n = I18n::builtin();
    let r = Render::new("s", "en", i18n);
    let mut best = f64::MAX;
    let mut size = 0;
    for _ in 0..20 {
        let start = Instant::now();
        let mut out = String::new();
        CodeView::new(CodeLang::Rust, text.clone()).render(&mut out, &r);
        best = best.min(start.elapsed().as_secs_f64() * 1000.0);
        size = out.len();
    }
    println!(
        "CodeView 5,000 lines ({} KB source): {best:.2} ms, {} KB HTML",
        text.len() / 1024,
        size / 1024
    );
    let json: serde_json::Value = (0..1_000)
        .map(|i| serde_json::json!({ "id": i, "name": format!("Item {i}") }))
        .collect();
    let start = Instant::now();
    let mut out = String::new();
    JsonView::new(&json).render(&mut out, &r);
    println!(
        "JsonView of 1,000 objects: {:.2} ms, {} KB HTML",
        start.elapsed().as_secs_f64() * 1000.0,
        out.len() / 1024
    );
}
