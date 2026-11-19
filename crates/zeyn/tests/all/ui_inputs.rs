//! Decimal, percent, masked and password inputs: the exact `Decimal` type, its rules, reading by
//! locale, rendering, memory totals and filters, and the server side mask check.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{mask_example, mask_fits, mask_raw};
use zeyn::{Aggregate, AggregateRequest, Component, Filter, I18n, Render, Rule, Sort, Value};

fn dec(s: &str) -> Decimal {
    Decimal::parse(s).unwrap()
}

#[test]
fn decimals_are_exact_and_compare_by_value() {
    let mut sum = Decimal::ZERO;
    for _ in 0..10 {
        sum = sum.checked_add(dec("0.1")).unwrap();
    }
    assert_eq!(sum, dec("1"), "ten times 0.1 is exactly 1");
    assert_eq!(dec("1.50"), dec("1.5"));
    assert_eq!(
        dec("1.50").to_string(),
        "1.50",
        "the scale is kept for display"
    );
    assert!(dec("-1.5") < dec("-1.2") && dec("-0.5") < dec("0.3") && dec("2") > dec("1.99"));
    assert_eq!(dec(".5"), dec("0.5"));
    assert_eq!(dec("12345E-2"), dec("123.45"));
    assert_eq!(dec("1.5e3"), dec("1500"));
    assert_eq!(
        dec("99999999999999999999999999999999999999").integer_digits(),
        38
    );
    assert!(
        Decimal::parse("999999999999999999999999999999999999999").is_none(),
        "39 digits do not fit"
    );
    assert!(
        Decimal::parse("1.2.3").is_none()
            && Decimal::parse("").is_none()
            && Decimal::parse("abc").is_none()
    );
    assert_eq!(dec("0.1250").fraction_digits(), 3);
    assert_eq!(dec("2.5").round(0), dec("3"));
    assert_eq!(dec("-2.5").round(0), dec("-3"));
    assert_eq!(dec("10").div_int(3, 4), Some(dec("3.3333")));
    assert_eq!(dec("2").div_int(3, 2), Some(dec("0.67")));
}

#[test]
fn decimals_read_and_write_in_the_users_language() {
    assert_eq!(
        Decimal::parse_input("1.234,5678", "tr"),
        Some(dec("1234.5678"))
    );
    assert_eq!(
        Decimal::parse_input("1,234.5678", "en"),
        Some(dec("1234.5678"))
    );
    assert_eq!(Decimal::parse_input("-0,5", "tr"), Some(dec("-0.5")));
    assert_eq!(
        Decimal::parse_input("1e5", "en"),
        None,
        "typed exponents are refused"
    );
    assert_eq!(dec("-1234567.25").format("tr"), "-1.234.567,25");
    assert_eq!(dec("-1234567.25").format("en"), "-1,234,567.25");
    assert_eq!(
        serde_json::to_value(dec("0.10")).unwrap(),
        serde_json::json!("0.10")
    );
    assert_eq!(
        serde_json::from_value::<Decimal>(serde_json::json!(0.1)).unwrap(),
        dec("0.1")
    );
    assert_eq!(
        serde_json::from_value::<Decimal>(serde_json::json!("3.25")).unwrap(),
        dec("3.25")
    );
    assert_eq!(
        serde_json::from_value::<Decimal>(serde_json::json!(7)).unwrap(),
        dec("7")
    );
}

#[entity(table = "rates")]
pub struct Rate {
    #[key]
    pub id: RateId,
    #[rule(precision = 8, scale = 4, min = 0, max = 1000)]
    pub factor: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
    pub plain: Decimal,
    #[rule(mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
    #[rule(min_len = 8)]
    pub password: String,
}

#[test]
fn decimal_rules_check_scale_precision_and_limits() {
    let ok = Rate {
        factor: dec("12.3456"),
        phone: "5321234567".into(),
        ..Rate::default()
    };
    assert!(Valid::new(ok.clone()).is_ok());
    let rules = |r: Rate| {
        Valid::new(r)
            .unwrap_err()
            .errors
            .into_iter()
            .map(|e| (e.field, e.rule))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        rules(Rate {
            factor: dec("1.23456"),
            ..ok.clone()
        }),
        [("factor", Rule::Scale(4))]
    );
    assert_eq!(
        rules(Rate {
            factor: dec("100000"),
            ..ok.clone()
        }),
        [("factor", Rule::Max(1000)), ("factor", Rule::Precision(8))]
    );
    assert_eq!(
        rules(Rate {
            factor: dec("-0.5"),
            ..ok.clone()
        }),
        [("factor", Rule::Min(0))]
    );
    assert_eq!(
        rules(Rate {
            discount: Some(dec("100.5")),
            ..ok.clone()
        }),
        [("discount", Rule::Max(100))]
    );
    assert_eq!(
        rules(Rate {
            plain: dec("0.001"),
            ..ok.clone()
        }),
        [("plain", Rule::Scale(2))],
        "2 decimals by default"
    );
    assert_eq!(
        rules(Rate {
            plain: dec("10000000000000000"),
            ..ok.clone()
        }),
        [("plain", Rule::Precision(18))],
        "18 digits by default, 16 before the separator"
    );
    let meta = &<Rate as zeyn::Model>::fields()[1];
    assert_eq!(meta.decimal_limits(), (8, 4));
    assert_eq!(<Rate as zeyn::Model>::fields()[3].decimal_limits(), (18, 2));
}

#[action(public)]
pub async fn save_rate(rate: Valid<Rate>) -> Result<Rate> {
    Ok(rate.into_inner())
}

fn form(rate: Rate) -> zeyn::SubmitForm<Rate, save_rate> {
    Form::of(rate)
        .field(Rate::factor)
        .field(Rate::discount)
        .control(Rate::discount, Control::Percent)
        .field(Rate::plain)
        .field(Rate::phone)
        .field(Rate::iban)
        .field(Rate::password)
        .control(Rate::password, Control::PasswordMeter)
        .submit("Save", save_rate)
}

fn html(locale: &str, rate: Rate) -> String {
    let mut out = String::new();
    form(rate).render(&mut out, &Render::new("r", locale, I18n::builtin()));
    out
}

#[test]
fn decimal_and_percent_inputs_keep_the_field_scale() {
    let rate = Rate {
        factor: dec("1234.5"),
        discount: Some(dec("12.5")),
        phone: "5321234567".into(),
        password: "secret-value".into(),
        ..Rate::default()
    };
    let tr = html("tr", rate.clone());
    assert!(
        tr.contains(r#"data-scale="4""#) && tr.contains(r#"value="1.234,5""#),
        "{tr}"
    );
    assert!(
        tr.contains(r#"class="z-number z-percent z-affix-first""#),
        "Turkish writes %12,5"
    );
    assert!(tr.contains(r#"aria-describedby="r-discount-unit r-discount-error""#));
    assert!(tr.contains(r#"<span class="z-sr-only" id="r-discount-unit">yüzde</span>"#));
    let en = html("en", rate);
    assert!(
        en.contains(r#"class="z-number z-percent" data-zeyn-number"#)
            && en.contains(r#"value="12.5""#)
    );
    assert!(en.contains(r#"inputmode="decimal""#));
}

#[test]
fn masked_inputs_hold_the_raw_value_and_password_is_never_rendered_back() {
    let rate = Rate {
        phone: "5321234567".into(),
        password: "secret-value".into(),
        ..Rate::default()
    };
    let en = html("en", rate);
    assert!(
        en.contains(r#"<div class="z-mask" data-zeyn-mask="(999) 999 99 99" data-example="(___) ___ __ __">"#),
        "{en}"
    );
    assert!(
        en.contains(r#"class="z-mask-value" dir="ltr" inputmode="numeric""#)
            && en.contains(r#"value="5321234567""#)
    );
    assert!(
        en.contains(r#"data-zeyn-mask="AA99 9999 9999 9999 9999 9999 99""#)
            && en.contains(r#"inputmode="text""#)
    );
    assert!(
        !en.contains("secret-value"),
        "passwords are never written into the page"
    );
    assert!(en.contains(r#"autocomplete="new-password""#));
    assert!(en.contains(
        r#"class="z-password-toggle" aria-pressed="false" aria-controls="r-password" aria-label="Show password""#
    ));
    assert!(en.contains(r#"aria-describedby="r-password-strength r-password-error""#));
    assert!(en.contains(r#"data-zeyn-strength="r-password""#) && en.contains("Strength: weak"));
}

#[test]
fn masks_accept_typed_literals_and_check_on_the_server() {
    assert_eq!(mask_raw("(999) 999 99 99", "(532) 123-45 67"), "5321234567");
    assert_eq!(mask_raw("AA99 9999", "tr33 0006"), "TR330006");
    assert_eq!(
        mask_raw("999", "12a"),
        "12a",
        "a letter in a digit slot is kept so the rule refuses it"
    );
    assert!(mask_fits("\\TR99", "33"), "escaped letters are literals");
    assert_eq!(mask_example("\\TR99"), "TR__");

    let f = form(Rate::default());
    let values = serde_json::json!({
        "factor": "234,5", "discount": "12,5", "plain": "3", "phone": "(532) 123 45 67",
        "iban": "tr33 0006 1005 1978 6457 8413 26", "password": "long enough"
    });
    let rate = f
        .form()
        .read(values.as_object().unwrap(), "tr", TimeZone::UTC)
        .unwrap();
    assert_eq!(rate.factor, dec("234.5"));
    assert_eq!(rate.factor.scale(), 4, "read values get the field scale");
    assert_eq!(rate.discount, Some(dec("12.5")));
    assert_eq!(rate.phone, "5321234567");
    assert_eq!(rate.iban.as_deref(), Some("TR330006100519786457841326"));

    let bad = serde_json::json!({ "factor": "1,23456", "phone": "532 12", "iban": "33TR", "plain": "x", "password": "short" });
    let errors = f
        .form()
        .read(bad.as_object().unwrap(), "tr", TimeZone::UTC)
        .unwrap_err();
    let got: Vec<(&str, Rule)> = errors
        .errors
        .iter()
        .map(|e| (e.field, e.rule))
        .collect();
    assert!(got.contains(&("factor", Rule::Scale(4))), "{got:?}");
    assert!(got.contains(&("phone", Rule::Mask("(999) 999 99 99"))));
    assert!(got.contains(&("iban", Rule::Mask("AA99 9999 9999 9999 9999 9999 99"))));
    assert!(got.contains(&("plain", Rule::Format)));
    assert!(got.contains(&("password", Rule::MinLen(8))));
    let phone = errors
        .errors
        .iter()
        .find(|e| e.field == "phone")
        .unwrap();
    assert_eq!(
        I18n::builtin().text("tr", &phone.message),
        "Phone (___) ___ __ __ biçiminde olmalı."
    );

    let api: Result<Valid<Rate>, _> = serde_json::from_value(
        serde_json::json!({ "id": 0, "factor": "1", "plain": "1", "phone": "(532) 123 45 67", "password": "" }),
    );
    assert!(
        api.is_err(),
        "API input is checked against the raw pattern too"
    );
}

#[test]
fn decimals_filter_sort_and_total_in_memory_without_floats() {
    let rows: Vec<Rate> = ["0.1", "0.2", "0.3", "10.05"]
        .iter()
        .enumerate()
        .map(|(i, f)| Rate {
            id: RateId(i as i64 + 1),
            factor: dec(f),
            ..Rate::default()
        })
        .collect();
    let q = DataQuery {
        filters: vec![Filter::range("factor", Some("0,15"), Some("10.05"))],
        sort: vec![Sort {
            field: "factor".into(),
            desc: true,
        }],
        aggregates: vec![
            AggregateRequest {
                field: "factor".into(),
                op: Aggregate::Sum,
            },
            AggregateRequest {
                field: "factor".into(),
                op: Aggregate::Avg,
            },
        ],
        ..DataQuery::default()
    };
    let page = q.apply(rows);
    let got: Vec<String> = page
        .items
        .iter()
        .map(|r| r.factor.to_string())
        .collect();
    assert_eq!(got, ["10.05", "0.3", "0.2"]);
    assert_eq!(page.totals[0].value, Value::Number(dec("10.55")));
    assert_eq!(
        page.totals[1].value,
        Value::Number(dec("3.52")),
        "average rounds to the values' scale"
    );
}

#[test]
fn decimal_fields_describe_themselves_for_apis_and_tools() {
    let schema = zeyn::model_schema::<Rate>();
    assert_eq!(schema["properties"]["factor"]["type"], "string");
    assert!(
        schema["properties"]["factor"]["description"]
            .as_str()
            .unwrap()
            .contains("8 digits and 4 decimals")
    );
    let tools = form(Rate::default());
    let tool = zeyn::FormSource::tool_schema(&tools);
    assert!(
        tool["properties"]["factor"]["description"]
            .as_str()
            .unwrap()
            .contains("at most 4 decimals")
    );
    assert!(
        tool["properties"]["phone"]["description"]
            .as_str()
            .unwrap()
            .contains("(999) 999 99 99")
    );
}

#[screen(route = "/rates", public)]
pub fn rates() -> Screen {
    Screen::new("Rates").body(form(Rate {
        phone: "5321234567".into(),
        ..Rate::default()
    }))
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn submit(values: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let app = App::new("Test")
        .i18n(I18n::new("en"))
        .feature(Feature::new("rates").screen(rates))
        .router()
        .unwrap();
    let req = Request::post("/_zeyn/form")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(
            serde_json::json!({ "screen": "rates", "node": "0", "values": values }).to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn the_form_endpoint_reads_decimals_and_refuses_bad_masks() {
    let (status, body) = submit(
        serde_json::json!({ "factor": "2.5", "plain": "1", "phone": "532 123 45 67", "password": "long enough" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) =
        submit(serde_json::json!({ "factor": "2.55555", "plain": "1", "phone": "532", "password": "long enough" }))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let fields: Vec<&str> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["field"].as_str())
        .collect();
    assert_eq!(fields, ["factor", "phone"]);
}
