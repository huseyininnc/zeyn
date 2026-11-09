//! Color and length controls, runtime field specs and schemas, and the island property
//! inspector: parsing, unit conversion, validation, one rendering path for model and runtime
//! fields, escaping of hostile schemas and the size caps.

use serde_json::json;
use zeyn::prelude::*;
use zeyn::ui::{
    FieldSpec, FormSchema, LengthUnit, MAX_FIELD_NAME, MAX_SCHEMA_FIELDS, MAX_SCHEMA_OPTIONS,
    MAX_SCHEMA_TEXT, SchemaForm, is_hex_color,
};
use zeyn::{Component, I18n, Render, Text};

const PALETTE: &[&str] = &["#1E88E5", "#43A047", "javascript:alert(1)", "#00000080"];

#[model]
pub struct Shape {
    #[rule(required, max_len = 40)]
    pub title: String,
    #[rule(required)]
    pub fill: String,
    #[rule(required, min = 0, max = 2000, scale = 4)]
    pub width: Option<Decimal>,
}

#[action(public)]
pub async fn save_shape(shape: Valid<Shape>) -> Result<String> {
    Ok(shape.fill.clone())
}

fn shape_form(shape: Shape) -> zeyn::SubmitForm<Shape, save_shape> {
    Form::of(shape)
        .field(Shape::title)
        .field(Shape::fill)
        .control(Shape::fill, Control::Color { palette: PALETTE })
        .field(Shape::width)
        .control(
            Shape::width,
            Control::Length {
                units: &[LengthUnit::Mm, LengthUnit::Pt],
            },
        )
        .submit("Save", save_shape)
}

fn render(node: &dyn Component, locale: &str) -> String {
    let mut out = String::new();
    node.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

fn shape() -> Shape {
    Shape {
        title: "Sales".into(),
        fill: "#1E88E5".into(),
        width: Decimal::parse("37.7953"),
    }
}

#[test]
fn lengths_convert_between_units_through_pixels() {
    assert_eq!(LengthUnit::In.to_px(1.0), 96.0);
    assert_eq!(LengthUnit::Pt.to_px(72.0), 96.0);
    assert!((LengthUnit::Mm.to_px(25.4) - 96.0).abs() < 1e-9);
    assert!((LengthUnit::Cm.to_px(2.54) - 96.0).abs() < 1e-9);
    assert_eq!(LengthUnit::Px.to_px(12.5), 12.5);
    for unit in LengthUnit::ALL {
        let back = unit.from_px(unit.to_px(12.34));
        assert!((back - 12.34).abs() < 1e-9, "{unit:?} round trip");
        assert_eq!(LengthUnit::parse(unit.code()), Some(*unit));
    }
    assert_eq!(LengthUnit::parse("em"), None);
}

#[test]
fn only_six_or_eight_hex_digits_are_a_color() {
    for ok in ["#000000", "#1e88e5", "#1E88E580", "#ABCDEF"] {
        assert!(is_hex_color(ok), "{ok}");
    }
    for bad in [
        "",
        "#",
        "#fff",
        "#12345",
        "#1234567",
        "#123456789",
        "#12345g",
        " #123456",
        "#123456 ",
        "123456",
        "red",
        "javascript:alert(1)",
        "#１２３４５６",
    ] {
        assert!(!is_hex_color(bad), "{bad:?}");
    }
}

#[test]
fn a_color_field_renders_a_hex_input_with_a_hidden_picker_button_and_a_clean_palette() {
    let html = render(&shape_form(shape()), "en");
    assert!(html.contains(r#"<div class="z-color" data-zeyn-color data-palette="[&quot;#1E88E5&quot;,&quot;#43A047&quot;,&quot;#00000080&quot;]""#), "palette entries that are not hex colors are dropped: {html}");
    assert!(html.contains(r#"class="z-color-hex" dir="ltr""#));
    assert!(html.contains(r#"id="s-fill" name="fill""#));
    assert!(html.contains(r##"value="#1E88E5""##));
    assert!(
        html.contains(
            r#"aria-haspopup="dialog" aria-expanded="false" aria-label="Choose a color""#
        )
    );
    assert!(html.contains(r#"data-testid="s.field.fill.picker" hidden>"#));
    assert!(!html.contains("javascript:"));
    let tr = render(&shape_form(shape()), "tr");
    assert!(tr.contains(r#"aria-label="Renk seç""#));
}

#[test]
fn the_server_takes_only_valid_hex_colors() {
    let f = shape_form(Shape::default());
    let ok = json!({ "title": "A", "fill": "#00FF0080", "width": "10" });
    let shape = f
        .form()
        .read(ok.as_object().unwrap(), "en", TimeZone::UTC)
        .unwrap();
    assert_eq!(shape.fill, "#00FF0080");
    for bad in ["red", "#fff", "#12345g", "\"><script>"] {
        let values = json!({ "title": "A", "fill": bad, "width": "10" });
        let errors = f
            .form()
            .read(values.as_object().unwrap(), "en", TimeZone::UTC)
            .unwrap_err();
        assert!(
            errors
                .errors
                .iter()
                .any(|e| e.field == "fill"),
            "{bad} is refused"
        );
    }
}

#[test]
fn a_length_field_keeps_pixels_in_a_native_input_and_lists_its_units() {
    let html = render(&shape_form(shape()), "tr");
    assert!(
        html.contains(r#"<div class="z-number z-length" data-zeyn-length data-units="[&quot;mm&quot;,&quot;pt&quot;]" data-decimal="," data-scale="4" data-min="0" data-max="2000" data-text-unit="Birim">"#),
        "{html}"
    );
    assert!(html.contains(r#"class="z-num z-length-px" id="s-width" name="width""#));
    assert!(
        html.contains(r#"value="37,7953""#),
        "pixels in the locale's format"
    );
    assert!(html.contains(r#"<span class="z-affix" aria-hidden="true">px</span>"#));
}

#[test]
fn the_server_reads_lengths_in_pixels_and_refuses_nan_and_infinity() {
    let f = shape_form(Shape::default());
    let ok = json!({ "title": "A", "fill": "#000000", "width": "37,7953" });
    let shape = f
        .form()
        .read(ok.as_object().unwrap(), "tr", TimeZone::UTC)
        .unwrap();
    assert_eq!(shape.width.unwrap().to_string(), "37.7953");
    for bad in ["NaN", "Infinity", "-inf", "1e400", "12px", "2001"] {
        let values = json!({ "title": "A", "fill": "#000000", "width": bad });
        let errors = f
            .form()
            .read(values.as_object().unwrap(), "en", TimeZone::UTC)
            .unwrap_err();
        assert!(
            errors
                .errors
                .iter()
                .any(|e| e.field == "width"),
            "{bad} is refused"
        );
    }
}

#[test]
fn model_fields_and_runtime_fields_render_through_the_same_path() {
    let typed = render(&shape_form(shape()), "en");
    let schema = FormSchema::new()
        .field(
            FieldSpec::text("title", "Title")
                .required()
                .max_len(40),
        )
        .field(FieldSpec::color("fill", "Fill", PALETTE).required());
    let runtime = render(
        &SchemaForm::new(schema, json!({ "title": "Sales", "fill": "#1E88E5" })),
        "en",
    )
    .replace("s-f0", "s-title")
    .replace("s-f1", "s-fill");
    let input = r#"<input type="text" id="s-title" name="title" data-testid="s.field.title" aria-describedby="s-title-error" required aria-required="true" maxlength="40" value="Sales">"#;
    assert!(typed.contains(input), "{typed}");
    assert!(runtime.contains(input), "{runtime}");
    let color = |html: &str| {
        let start = html
            .find(r#"<div class="z-color""#)
            .unwrap();
        let end = start + html[start..].find("</div>").unwrap();
        html[start..end].to_string()
    };
    assert_eq!(color(&typed), color(&runtime));
}

#[test]
fn field_specs_check_types_ranges_and_lengths() {
    let width = FieldSpec::length("w", "Width", &[])
        .min(0)
        .max(100);
    assert_eq!(width.check(&json!(12.5)).unwrap(), json!(12.5));
    assert_eq!(width.check(&json!(null)).unwrap(), json!(null));
    for bad in [
        json!(-1),
        json!(101),
        json!("NaN"),
        json!("12"),
        json!(true),
    ] {
        assert!(width.check(&bad).is_err(), "{bad}");
    }
    let count = FieldSpec::integer("n", "Count").required();
    assert!(count.check(&json!(3)).is_ok());
    assert!(count.check(&json!(3.5)).is_err());
    assert!(count.check(&json!(null)).is_err());
    let name = FieldSpec::text("name", "Name").max_len(3);
    assert!(name.check(&json!("abç")).is_ok(), "characters, not bytes");
    assert!(name.check(&json!("abcd")).is_err());
    assert!(name.check(&json!(5)).is_err());
    let fill = FieldSpec::color("fill", "Fill", &[]);
    assert!(fill.check(&json!("#FFFFFF")).is_ok());
    assert!(fill.check(&json!("white")).is_err());
    let align = FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]);
    assert!(align.check(&json!("left")).is_ok());
    assert!(align.check(&json!("center")).is_err());
    let bold = FieldSpec::boolean("bold", "Bold");
    assert_eq!(bold.check(&json!(false)).unwrap(), json!(false));
    assert!(bold.check(&json!("yes")).is_err());

    let schema = FormSchema::new()
        .field(width)
        .field(FieldSpec::text("t", "T").default("x"));
    let values = json!({ "w": 50, "unknown": 1 });
    let typed = schema
        .check(values.as_object().unwrap())
        .unwrap();
    assert_eq!(json!(typed), json!({ "w": 50, "t": "x" }));
    let bad = json!({ "w": 500 });
    let errors = schema
        .check(bad.as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].0, "w");
}

#[test]
fn schemas_cap_fields_names_texts_and_options() {
    let mut schema = FormSchema::new();
    for i in 0..MAX_SCHEMA_FIELDS + 50 {
        schema = schema.field(FieldSpec::text(&format!("f{i}"), "F"));
    }
    assert_eq!(schema.len(), MAX_SCHEMA_FIELDS);
    let long = "x".repeat(MAX_SCHEMA_TEXT * 3);
    let spec = FieldSpec::text(&"n".repeat(500), long.clone()).help(long.clone());
    assert_eq!(spec.name().len(), MAX_FIELD_NAME);
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json["label"].as_str().unwrap().len(), MAX_SCHEMA_TEXT);
    assert_eq!(json["help"].as_str().unwrap().len(), MAX_SCHEMA_TEXT);
    let many = FieldSpec::choice(
        "c",
        "C",
        (0..1000).map(|i| (format!("v{i}"), format!("V{i}"))),
    );
    let json = serde_json::to_value(&many).unwrap();
    assert_eq!(
        json["options"]
            .as_array()
            .unwrap()
            .len(),
        MAX_SCHEMA_OPTIONS
    );
}

#[test]
fn a_hostile_schema_renders_as_text() {
    let evil = r#""><script>alert(1)</script><img src=x onerror=alert(1)>'"#;
    let schema = FormSchema::new()
        .section(evil)
        .field(
            FieldSpec::text(r#"a"onfocus="x"#, evil)
                .help(evil)
                .default(evil),
        )
        .field(FieldSpec::choice("pick", evil, [(evil, evil)]).default(evil))
        .field(FieldSpec::multiline("note", evil).default(evil));
    let html = render(&SchemaForm::new(schema, json!({})), "en");
    for raw in [
        "<script",
        "<img",
        r#"onerror=alert(1)>"#,
        r#"" onfocus"#,
        r#""onfocus"#,
    ] {
        assert!(!html.contains(raw), "{raw} leaked: {html}");
    }
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(
        html.contains(r#"name="a_onfocus__x""#),
        "names keep only safe characters"
    );
    let json = serde_json::to_string(&FormSchema::new().field(FieldSpec::text("t", evil))).unwrap();
    assert!(
        json.contains("<script>"),
        "the schema JSON keeps the text as data"
    );
}

#[test]
fn the_island_inspector_ships_one_template_per_kind_and_no_inline_code() {
    let inspector = PropertyInspector::island("designer");
    let html = render(&inspector, "en");
    assert!(html.contains(r#"data-zeyn-inspector="designer""#));
    assert!(html.contains(r#"data-limits="{&quot;fields&quot;:200"#));
    for kind in [
        "text",
        "multiline",
        "integer",
        "decimal",
        "length",
        "color",
        "boolean",
        "choice",
        "expression",
    ] {
        assert!(
            html.contains(&format!(
                r#"<template data-kind="{kind}"><div class="z-field">"#
            )),
            "{kind}"
        );
    }
    assert!(html.contains(r#"id="s-zeyn-field-name" name="zeyn-field-name""#));
    assert!(html.contains("Select an item to see its properties."));
    assert!(html.contains(r#"aria-live="polite" data-zeyn-inspector-live"#));
    for inline in ["<script", " style=", " onclick", " onerror", " onchange"] {
        assert!(!html.contains(inline), "{inline}");
    }
    let tr = render(&inspector, "tr");
    assert!(tr.contains("Özellikler"));
    assert_eq!(inspector.schema()["island"], json!("designer"));
    assert!(inspector.form().is_none(), "nothing posts to /_zeyn/form");
}

#[test]
fn rendering_an_inspector_with_fifty_fields_stays_cheap() {
    let mut schema = FormSchema::new().section("Properties");
    for i in 0..50 {
        schema = schema.field(match i % 5 {
            0 => FieldSpec::text(&format!("t{i}"), "Text"),
            1 => FieldSpec::length(&format!("l{i}"), "Length", &[LengthUnit::Mm]),
            2 => FieldSpec::color(&format!("c{i}"), "Color", PALETTE),
            3 => FieldSpec::boolean(&format!("b{i}"), "Flag"),
            _ => FieldSpec::choice(&format!("h{i}"), "Pick", [("a", "A"), ("b", "B")]),
        });
    }
    let form = SchemaForm::new(schema, json!({}));
    let start = std::time::Instant::now();
    let html = render(&form, "en");
    let elapsed = start.elapsed();
    assert_eq!(
        html.matches(r#"<div class="z-field">"#)
            .count(),
        50
    );
    assert!(elapsed.as_millis() < 50, "{elapsed:?}");
}

#[test]
fn runtime_field_ids_come_from_their_place_so_names_cannot_collide() {
    let schema = FormSchema::new()
        .field(FieldSpec::text("x", "X").help("Help"))
        .field(FieldSpec::text("x-error", "X error").help("Help"))
        .field(FieldSpec::text("x-help", "X help"));
    let html = render(&SchemaForm::new(schema, json!({})), "en");
    let ids: Vec<&str> = html
        .split(r#" id=""#)
        .skip(1)
        .map(|s| s.split('"').next().unwrap())
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "{ids:?}");
    assert!(html.contains(r#"id="s-f1" name="x-error""#), "{html}");
}

#[test]
fn a_boolean_without_a_value_or_default_is_false() {
    let schema = FormSchema::new().field(FieldSpec::boolean("bold", "Bold"));
    let typed = schema
        .check(&serde_json::Map::new())
        .unwrap();
    assert_eq!(typed["bold"], json!(false));
}

#[test]
fn schema_texts_follow_the_users_language() {
    let schema = FormSchema::new()
        .section(Text::key("zeyn.ui.inspector.title"))
        .field(
            FieldSpec::choice(
                "c",
                Text::key("zeyn.ui.inspector.title"),
                [("a", Text::key("zeyn.ui.color.red"))],
            )
            .help(Text::key("zeyn.ui.color.blue")),
        );
    let tr = schema.to_json(I18n::builtin(), "tr");
    assert_eq!(tr["sections"][0]["title"], "Özellikler");
    let field = &tr["sections"][0]["fields"][0];
    assert_eq!(field["label"], "Özellikler");
    assert_eq!(field["help"], "Mavi");
    assert_eq!(field["options"][0]["label"], "Kırmızı");
    let en = serde_json::to_value(&schema).unwrap();
    assert_eq!(en["sections"][0]["fields"][0]["label"], "Properties");
}

#[test]
fn decimals_and_lengths_keep_their_scale() {
    let width = FieldSpec::length("w", "Width", &[]);
    assert!(width.check(&json!(37.7953)).is_ok());
    assert!(width.check(&json!(37.79531)).is_err());
    assert!(
        width.check(&json!(1e-7)).is_err(),
        "exponent numbers past the scale"
    );
    assert!(width.check(&json!(0.0001)).is_ok());
    let json = serde_json::to_value(&width).unwrap();
    assert_eq!(json["scale"], json!(4));
}

#[test]
fn builders_drop_duplicate_names_crossed_limits_and_long_option_values() {
    let schema = FormSchema::new()
        .field(FieldSpec::text("a", "A"))
        .field(FieldSpec::integer("a", "Again"));
    assert_eq!(schema.len(), 1);
    let n = FieldSpec::integer("n", "N")
        .min(10)
        .max(5);
    assert!(
        n.check(&json!(20)).is_ok(),
        "a max below the min is ignored"
    );
    assert!(n.check(&json!(9)).is_err());
    let long = "v".repeat(MAX_SCHEMA_TEXT + 1);
    let c = FieldSpec::choice("c", "C", [(long.as_str(), "Long"), ("ok", "Ok")]);
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(
        json["options"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
