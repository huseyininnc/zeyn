//! PDF documents through the HTTP endpoints: a `PdfLink` rendered from an action's data, a grid
//! list report, permissions and configuration.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Config, Identity, Pdf, PdfLink, PdfTemplate, StaticSource};

async fn config(pairs: &[(&str, &str)]) -> Config {
    Config::load(vec![Box::new(StaticSource(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    ))])
    .await
    .unwrap()
}

/// Invoice line used by the PDF tests.
#[entity(table = "lines")]
pub struct Line {
    #[key]
    pub id: LineId,
    #[rule(required)]
    pub item: String,
    pub qty: i64,
    pub price: Money,
}

#[derive(Clone, Default)]
pub struct Lines(Arc<RwLock<Vec<Line>>>);

impl Service for Lines {}

#[action(permission = "bills.read")]
pub async fn search_lines(q: DataQuery, store: Lines) -> Result<Page<Line>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

/// Which bill a document shows.
#[params]
pub struct BillParams {
    pub id: i64,
}

#[derive(serde::Serialize)]
pub struct BillData {
    pub number: String,
    pub customer: String,
    pub lines: Vec<(String, i64, String)>,
}

#[action(permission = "bills.print")]
pub async fn bill_data(p: BillParams, store: Lines) -> Result<BillData> {
    let lines = store
        .0
        .read()
        .unwrap()
        .iter()
        .map(|l| (l.item.clone(), l.qty, l.price.format("en")))
        .collect();
    Ok(BillData {
        number: format!("B-{}", p.id),
        customer: "#panic(\"no\") Öz Ltd.".into(),
        lines,
    })
}

const BILL: PdfTemplate = PdfTemplate::new(
    "bill",
    "#set text(font: \"Libertinus Serif\")\n= Bill #data.number\n#data.customer\n#table(columns: 3, ..data.lines.flatten().map(v => str(v)))",
);

#[screen(route = "/bills", permission = "bills.read")]
pub fn bills(p: BillParams) -> Screen {
    Screen::new("Bills")
        .toolbar(|t| t.export(Export::Pdf))
        .body(
            DataGrid::of(search_lines)
                .column(Line::item)
                .column(Line::qty)
                .column(Line::price)
                .exportable(),
        )
        .body(PdfLink::new("Print", BILL, bill_data, p))
}

fn app(permissions: &[&'static str], rows: i64) -> Router {
    let identity = Identity::user("ayse").with_permissions(permissions.iter().copied());
    let lines = (1..=rows)
        .map(|i| Line {
            id: LineId(i),
            item: format!("Item {i} ]#x"),
            qty: i,
            price: Money::tl(i * 1_234),
        })
        .collect();
    App::new("Test")
        .service(Lines(Arc::new(RwLock::new(lines))))
        .service(Pdf::from_config(&Config::default()).unwrap())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("bills").screen(bills))
        .router()
        .unwrap()
}

async fn get(router: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let req = Request::get(uri)
        .header("x-user", "1")
        .header(header::COOKIE, "zeyn_locale=tr")
        .body(Body::empty())
        .unwrap();
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (
        status,
        headers,
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}

const PARAMS: &str = "params=%7B%22id%22%3A7%7D";

#[tokio::test]
async fn pdf_links_render_the_actions_data_for_users_who_may_run_it() {
    let router = app(&["bills.read", "bills.print"], 3);
    let (_, _, page) = get(&router, &format!("/bills?{PARAMS}")).await;
    let html = String::from_utf8(page).unwrap();
    assert!(
        html.contains(r#"data-zeyn-pdf data-testid="bills.pdf.bill">Print</button>"#),
        "{html}"
    );
    let (status, headers, body) =
        get(&router, &format!("/_zeyn/pdf?screen=bills&node=1&{PARAMS}")).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("bill.pdf")
    );
    assert!(body.starts_with(b"%PDF-"));

    let reader = app(&["bills.read"], 3);
    let (_, _, page) = get(&reader, &format!("/bills?{PARAMS}")).await;
    assert!(
        !String::from_utf8(page)
            .unwrap()
            .contains("data-zeyn-pdf")
    );
    let (status, ..) = get(&reader, &format!("/_zeyn/pdf?screen=bills&node=1&{PARAMS}")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, ..) = get(&router, &format!("/_zeyn/pdf?screen=bills&node=0&{PARAMS}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a grid is not a document");
}

#[tokio::test]
async fn grids_print_a_list_report() {
    let router = app(&["bills.read"], 40);
    let (_, _, page) = get(&router, &format!("/bills?{PARAMS}")).await;
    let html = String::from_utf8(page).unwrap();
    assert!(
        html.contains(r#"data-zeyn-export="pdf""#) && html.contains("pdf\""),
        "{html}"
    );
    let (status, headers, body) = get(
        &router,
        &format!("/_zeyn/grid/export?screen=bills&node=0&format=pdf&{PARAMS}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("bills.pdf")
    );
    assert!(body.starts_with(b"%PDF-"));
    let anonymous = Request::get(format!(
        "/_zeyn/grid/export?screen=bills&node=0&format=pdf&{PARAMS}"
    ))
    .body(Body::empty())
    .unwrap();
    assert_ne!(
        router
            .clone()
            .oneshot(anonymous)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn configuration_sets_fonts_and_the_concurrency_limit() {
    assert_eq!(
        Pdf::from_config(&config(&[("pdf.max_concurrent", "3")]).await)
            .unwrap()
            .max_concurrent(),
        3
    );
    assert!(Pdf::from_config(&config(&[("pdf.max_concurrent", "0")]).await).is_err());
    assert!(Pdf::from_config(&config(&[("pdf.fonts", "/no/such/dir")]).await).is_err());
    assert!(Pdf::new().max_concurrent() >= 1);
}

#[tokio::test]
async fn documents_wait_for_a_free_slot() {
    let pdf = Pdf::from_config(&config(&[("pdf.max_concurrent", "1")]).await).unwrap();
    let data = serde_json::json!({ "name": "x" });
    let t = PdfTemplate::new("t", "#data.name");
    let started = std::time::Instant::now();
    let (a, b, c) = tokio::join!(
        pdf.render(t, &data),
        pdf.render(t, &data),
        pdf.render(t, &data)
    );
    assert!(a.unwrap().starts_with(b"%PDF") && b.is_ok() && c.is_ok());
    assert!(started.elapsed().as_secs() < 60);
}
