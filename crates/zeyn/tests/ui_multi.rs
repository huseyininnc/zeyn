//! Multi select for `Vec` of a `#[choice]` and lookups that show several columns: markup with a
//! native fallback, server reading of the posted values, schemas and the lookup's access check.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, FieldKind, FieldType, Filter, I18n, Identity, Render, Rule};

#[choice]
pub enum Channel {
    Email,
    Phone,
    #[label("Text message")]
    Sms,
    Post,
}

#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    pub name: String,
}

#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
    pub also: Vec<Channel>,
    pub customer: Option<CustomerId>,
}

#[action(permission = "customers.read")]
pub async fn find_customers(q: LookupQuery) -> Result<Page<LookupItem>> {
    let all = [
        ("C-1", "Ayşe Yılmaz", "İzmir"),
        ("C-2", "Mert Kaya", "Ankara"),
        ("C-3", "İnci Gıda", "Bursa"),
    ];
    Ok(q.apply(
        all.iter()
            .enumerate()
            .map(|(i, (code, name, city))| {
                LookupItem::new(i as i64 + 1, *name).cells([*code, *name, *city])
            })
            .collect(),
    ))
}

#[action(public)]
pub async fn save_contact(contact: Valid<Contact>) -> Result<Contact> {
    Ok(contact.into_inner())
}

fn form(contact: Contact) -> zeyn::SubmitForm<Contact, save_contact> {
    Form::of(contact)
        .field(Contact::channels)
        .field(Contact::also)
        .lookup_columns(Contact::customer, find_customers, ["Code", "Name", "City"])
        .submit("Save", save_contact)
}

#[test]
fn vec_of_a_choice_is_a_multi_select_over_a_native_multiple_select() {
    assert!(matches!(<Vec<Channel> as FieldType>::KIND, FieldKind::Choices(o) if o.len() == 4));
    let mut out = String::new();
    form(Contact {
        channels: vec![Channel::Email, Channel::Sms],
        ..Contact::default()
    })
    .render(&mut out, &Render::new("c", "tr", I18n::builtin()));
    assert!(
        out.contains(r#"<div class="z-multi" data-zeyn-multi data-texts="#),
        "{out}"
    );
    assert!(out.contains(r#"<select multiple size="4" id="c-channels" name="channels""#));
    assert!(out.contains(r#"<option value="Email" selected>Email</option>"#));
    assert!(out.contains(r#"<option value="Sms" selected>Text message</option>"#));
    assert!(out.contains(r#"<option value="Phone">Phone</option>"#));
    assert!(
        out.contains("{n} seçili"),
        "texts come in the user's language"
    );
}

#[test]
fn posted_choices_are_checked_one_by_one() {
    let f = form(Contact::default());
    let values = serde_json::json!({ "channels": "Sms,Email,Sms", "also": ["Post"] });
    let c = f
        .form()
        .read(values.as_object().unwrap(), "en", TimeZone::UTC)
        .unwrap();
    assert_eq!(
        c.channels,
        vec![Channel::Sms, Channel::Email],
        "repeats are dropped, order is kept"
    );
    assert_eq!(c.also, vec![Channel::Post]);

    let bad = serde_json::json!({ "channels": "Email,Fax", "also": [1] });
    let errors = f
        .form()
        .read(bad.as_object().unwrap(), "en", TimeZone::UTC)
        .unwrap_err();
    let got: Vec<(&str, Rule)> = errors
        .errors
        .iter()
        .map(|e| (e.field, e.rule))
        .collect();
    assert_eq!(got, [("channels", Rule::Format), ("also", Rule::Format)]);

    let empty = serde_json::json!({ "channels": "", "also": "" });
    let errors = f
        .form()
        .read(empty.as_object().unwrap(), "en", TimeZone::UTC)
        .unwrap_err();
    assert_eq!(errors.errors[0].rule, Rule::Required);
}

#[test]
fn several_choices_show_their_labels_and_describe_themselves() {
    let r = Render::english("c");
    let v = <Vec<Channel> as FieldType>::to_value(&vec![Channel::Sms, Channel::Post]);
    assert_eq!(v, zeyn::Value::Text("Sms,Post".into()));
    assert_eq!(
        r.value(&v, &<Vec<Channel> as FieldType>::KIND),
        "Text message, Post"
    );
    let schema = zeyn::model_schema::<Contact>();
    assert_eq!(schema["properties"]["channels"]["type"], "array");
    assert_eq!(schema["properties"]["channels"]["items"]["enum"][2], "Sms");
    let json = serde_json::to_value(Contact {
        channels: vec![Channel::Phone],
        ..Contact::default()
    })
    .unwrap();
    assert_eq!(json["channels"], serde_json::json!(["Phone"]));
    let rows = vec![
        Contact {
            channels: vec![Channel::Phone],
            ..Contact::default()
        },
        Contact {
            channels: vec![Channel::Email, Channel::Post],
            ..Contact::default()
        },
    ];
    let page = DataQuery {
        filters: vec![Filter::contains("channels", "post")],
        ..DataQuery::default()
    }
    .apply(rows);
    assert_eq!(
        page.items.len(),
        1,
        "a contains filter looks into the chosen values"
    );
}

#[test]
fn multi_column_lookups_send_headers_and_search_every_cell() {
    let mut out = String::new();
    form(Contact::default()).render(&mut out, &Render::new("c", "en", I18n::builtin()));
    assert!(
        out.contains(r#"data-zeyn-lookup="customer" data-multiple="false" data-columns="[&quot;Code&quot;,&quot;Name&quot;,&quot;City&quot;]""#),
        "{out}"
    );
    let q = LookupQuery {
        text: "bursa".into(),
        limit: 10,
        ..Default::default()
    };
    let items = vec![LookupItem::new(3, "İnci Gıda").cells(["C-3", "İnci Gıda", "Bursa"])];
    assert_eq!(q.apply(items).items.len(), 1, "cells are searched too");
    let many = LookupItem::new(1, "x").cells(["1", "2", "3", "4", "5", "6", "7", "8"]);
    assert_eq!(many.cells.len(), zeyn::ui::MAX_LOOKUP_COLUMNS);
    let plain = serde_json::to_value(LookupItem::new(1, "x")).unwrap();
    assert!(plain.get("cells").is_none(), "plain lookups send no cells");
}

#[screen(route = "/contact", public)]
pub fn contacts() -> Screen {
    Screen::new("Contact").body(Panel::new("Contact").add(form(Contact::default())))
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn post(path: &str, perms: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let app = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("contacts").screen(contacts))
        .router()
        .unwrap();
    let req = Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
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
async fn the_column_lookup_searches_through_the_actions_permission() {
    let call = serde_json::json!({ "screen": "contacts", "node": "0.0", "key": "customer", "input": { "text": "ank", "limit": 5 } });
    let (status, body) = post("/_zeyn/data", "customers.read", call.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["items"][0]["cells"],
        serde_json::json!(["C-2", "Mert Kaya", "Ankara"])
    );
    let (status, _) = post("/_zeyn/data", "", call).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_form_endpoint_takes_the_joined_values_the_runtime_posts() {
    let ok = serde_json::json!({ "screen": "contacts", "node": "0.0", "values": { "channels": "Phone,Post", "also": "" } });
    let (status, body) = post("/_zeyn/form", "", ok).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bad = serde_json::json!({ "screen": "contacts", "node": "0.0", "values": { "channels": "Phone,<b>", "also": "" } });
    let (status, body) = post("/_zeyn/form", "", bad).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["errors"][0]["field"], "channels");
}
