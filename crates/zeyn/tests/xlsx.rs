//! XLSX export and import through the HTTP endpoints: cell types, the user's layout, language and
//! time zone, a round trip, limits and broken files.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use calamine::{Data, Reader};
use http_body_util::BodyExt;
use std::io::Write;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity};

/// Row used by the XLSX tests.
#[entity(table = "parts")]
pub struct Part {
    #[key]
    pub id: PartId,
    #[rule(required, max_len = 40)]
    pub code: String,
    pub kind: Sort,
    #[rule(min = 0)]
    pub price: Money,
    pub qty: i64,
    pub active: bool,
    pub added: Date,
    pub seen: DateTime,
    #[mask(last4)]
    pub iban: String,
}

/// Part sort.
#[choice]
pub enum Sort {
    Tool,
    Spare,
}

#[derive(Clone, Default)]
pub struct Parts(Arc<RwLock<Vec<Part>>>);

impl Service for Parts {}

fn part(i: i64) -> Part {
    Part {
        id: PartId(i),
        code: format!("P-{i:03}"),
        kind: if i % 2 == 0 { Sort::Tool } else { Sort::Spare },
        price: Money::new(i * 1_050, if i % 3 == 0 { Currency::Usd } else { Currency::Try }),
        qty: i * 10,
        active: i % 2 == 0,
        added: Date::new(2026, 1 + (i % 12) as u8, 1 + (i % 28) as u8).unwrap(),
        seen: DateTime::from_unix_ms(1_767_225_600_000 + i * 3_600_000),
        iban: format!("TR00000000000000000000{i:04}"),
    }
}

impl Parts {
    fn seeded(n: i64) -> Self {
        Parts(Arc::new(RwLock::new((1..=n).map(part).collect())))
    }
}

#[action(permission = "parts.read")]
pub async fn search_parts(q: DataQuery, store: Parts) -> Result<Page<Part>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "parts.import")]
pub async fn import_parts(rows: Vec<Valid<Part>>, store: Parts) -> Result<usize> {
    let mut all = store.0.write().unwrap();
    let n = rows.len();
    for (i, row) in rows.into_iter().enumerate() {
        let mut p = row.into_inner();
        p.id = PartId(1_000 + i as i64);
        all.push(p);
    }
    Ok(n)
}

#[screen(route = "/parts", permission = "parts.read")]
pub fn parts() -> Screen {
    Screen::new("Parts")
        .toolbar(|t| t.export(Export::Xlsx))
        .body(
            DataGrid::of(search_parts)
                .paging(Paging::pages(5))
                .column(Part::code)
                .column(Part::kind)
                .column(Part::price)
                .column(Part::qty)
                .column(Part::active)
                .column(Part::added)
                .column(Part::seen)
                .column(Part::iban)
                .exportable(),
        )
        .body(Import::of(import_parts).max_rows(50))
}

fn app(store: Parts) -> Router {
    let identity = Identity::user("ayse").with_permissions(["parts.read", "parts.import"]);
    App::new("Test")
        .service(store)
        .i18n(
            I18n::new("en")
                .catalog("tr", r#"{ "Part": { "code": "Kod", "price": "Fiyat" }, "Sort": { "Tool": "Alet" } }"#)
                .unwrap(),
        )
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("parts").screen(parts))
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

async fn export(router: &Router, q: serde_json::Value, locale: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let req = Request::get(format!("/_zeyn/grid/export?screen=parts&node=0&format=xlsx&q={}", enc(&q.to_string())))
        .header("x-user", "1")
        .header(header::COOKIE, format!("zeyn_locale={locale}; zeyn_tz=Europe%2FIstanbul"))
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

async fn import(router: &Router, xlsx: &[u8], commit: bool, locale: &str) -> (StatusCode, serde_json::Value) {
    let body = serde_json::json!({ "screen": "parts", "node": 1, "xlsx": base64(xlsx), "commit": commit });
    let req = Request::post("/_zeyn/import")
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}; zeyn_locale={locale}; zeyn_tz=Europe%2FIstanbul"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or_default())
}

fn sheet(bytes: &[u8]) -> calamine::Range<Data> {
    let mut book = calamine::Xlsx::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let name = book.sheet_names()[0].clone();
    book.worksheet_range(&name).unwrap()
}

#[tokio::test]
async fn export_keeps_cell_types_layout_labels_and_time_zone() {
    let router = app(Parts::seeded(12));
    let q = serde_json::json!({ "sort": [{ "field": "qty", "desc": true }], "columns": [{ "field": "price" }, { "field": "code", "width": 180 }, { "field": "kind" }, { "field": "qty" }, { "field": "active" }, { "field": "added" }, { "field": "seen" }, { "field": "iban" }] });
    let (status, headers, body) = export(&router, q, "tr").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
    assert!(headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains("parts.xlsx"));
    let range = sheet(&body);
    assert_eq!(range.height(), 13, "header and every row, not only the first page of 5");
    let head: Vec<String> = (0..8).map(|c| range.get((0, c)).unwrap().to_string()).collect();
    assert_eq!(head, ["Fiyat", "Kod", "Kind", "Qty", "Active", "Added", "Seen", "Iban"]);
    let row = |r: usize, c: usize| range.get((r, c)).unwrap().clone();
    assert_eq!(row(1, 0), Data::Float(126.0), "money as a number: 12 * 10.50, in USD");
    assert_eq!(row(1, 1), Data::String("P-012".into()));
    assert_eq!(row(1, 2), Data::String("Alet".into()), "choices by label in the user's language");
    assert_eq!(row(1, 3), Data::Float(120.0));
    assert_eq!(row(1, 4), Data::Bool(true));
    let Data::DateTime(added) = row(1, 5) else { panic!("{:?}", row(1, 5)) };
    let (y, m, d, ..) = added.to_ymd_hms_milli();
    assert_eq!((y, m, d), (2026, 1, 13));
    let Data::DateTime(seen) = row(1, 6) else { panic!("{:?}", row(1, 6)) };
    let (y, m, d, h, ..) = seen.to_ymd_hms_milli();
    assert_eq!((y, m, d, h), (2026, 1, 1, 15), "12:00 UTC is 15:00 in Istanbul");
    assert_eq!(row(1, 7), Data::String("•••• 0012".into()), "masked fields stay masked");
    let styles = String::from_utf8_lossy(&unzip(&body, "xl/styles.xml")).to_string();
    assert!(styles.contains(r##"formatCode="#,##0.00 &quot;$&quot;"##) && styles.contains("dd.mm.yyyy"), "{styles}");
    let xml = String::from_utf8_lossy(&unzip(&body, "xl/worksheets/sheet1.xml")).to_string();
    assert!(
        xml.contains(r#"<pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/>"#),
        "frozen header"
    );
    assert!(xml.contains(r#"<col min="2" max="2" width="25"#), "the user's column width: {xml}");
}

fn unzip(bytes: &[u8], name: &str) -> Vec<u8> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    zip.by_name(name).unwrap().read_to_end(&mut out).unwrap();
    out
}

#[tokio::test]
async fn text_that_looks_like_a_formula_stays_text() {
    let store = Parts::seeded(3);
    store.0.write().unwrap()[0].code = "=HYPERLINK(\"x\")".into();
    let router = app(store);
    let (_, _, body) = export(&router, serde_json::json!({ "columns": [{ "field": "code" }] }), "en").await;
    let xml = String::from_utf8_lossy(&unzip(&body, "xl/worksheets/sheet1.xml")).to_string();
    assert!(!xml.contains("<f>"), "{xml}");
    assert!(xml.contains("=HYPERLINK(\"x\")") || xml.contains("=HYPERLINK(&quot;x&quot;)"), "{xml}");
}

#[tokio::test]
async fn exported_files_import_again_with_the_same_values() {
    let store = Parts::seeded(20);
    let router = app(store.clone());
    for locale in ["tr", "en"] {
        let q = serde_json::json!({ "columns": [{ "field": "code" }, { "field": "kind" }, { "field": "price" }, { "field": "qty" }, { "field": "active" }, { "field": "added" }, { "field": "seen" }] });
        let (_, _, body) = export(&router, q, locale).await;
        let (status, j) = import(&router, &body, false, locale).await;
        assert_eq!(status, StatusCode::OK, "{j}");
        assert_eq!((j["total"].as_u64(), j["valid"].as_u64()), (Some(20), Some(20)), "{j}");
    }
    let (_, _, body) = export(&router, serde_json::json!({ "columns": [{ "field": "code" }, { "field": "kind" }, { "field": "price" }, { "field": "qty" }, { "field": "active" }, { "field": "added" }, { "field": "seen" }] }), "tr").await;
    let (status, j) = import(&router, &body, true, "tr").await;
    assert_eq!((status, j["imported"].as_u64()), (StatusCode::OK, Some(20)), "{j}");
    let all = store.0.read().unwrap().clone();
    for (old, new) in all[..20].iter().zip(&all[20..]) {
        assert_eq!(
            (&old.code, old.kind, old.qty, old.active, old.added, old.seen),
            (&new.code, new.kind, new.qty, new.active, new.added, new.seen)
        );
        assert_eq!(old.price.minor, new.price.minor);
    }
}

#[tokio::test]
async fn imports_report_row_errors_and_enforce_limits() {
    let router = app(Parts::seeded(1));
    let mut book = rust_xlsxwriter::Workbook::new();
    let s = book.add_worksheet();
    for (c, h) in ["code", "qty", "price", "added"].iter().enumerate() {
        s.write(0, c as u16, *h).unwrap();
    }
    s.write(1, 0, "OK-1").unwrap();
    s.write(1, 1, 5).unwrap();
    s.write(1, 2, 12.5).unwrap();
    let date = rust_xlsxwriter::Format::new().set_num_format("yyyy-mm-dd");
    for r in 1..=3 {
        s.write_datetime_with_format(r, 3, rust_xlsxwriter::ExcelDateTime::from_ymd(2026, 2, 3).unwrap(), &date)
            .unwrap();
    }
    s.write(2, 0, "").unwrap();
    s.write(2, 1, "many").unwrap();
    s.write(3, 0, "OK-3").unwrap();
    s.write(3, 1, 1.5).unwrap();
    let (status, j) = import(&router, &book.save_to_buffer().unwrap(), false, "en").await;
    assert_eq!(status, StatusCode::OK, "{j}");
    assert_eq!((j["total"].as_u64(), j["valid"].as_u64(), j["invalid"].as_u64()), (Some(3), Some(1), Some(2)), "{j}");
    let errors: Vec<(u64, &str)> = j["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["row"].as_u64().unwrap(), e["field"].as_str().unwrap()))
        .collect();
    assert!(
        errors.contains(&(2, "code")) && errors.contains(&(2, "qty")) && errors.contains(&(3, "qty")),
        "{errors:?}"
    );
    assert!(j["report"].as_str().unwrap().starts_with("\u{feff}Row;Field;Message"));

    let mut book = rust_xlsxwriter::Workbook::new();
    let s = book.add_worksheet();
    s.write(0, 0, "code").unwrap();
    for r in 1..=51u32 {
        s.write(r, 0, format!("R{r}")).unwrap();
    }
    let (status, j) = import(&router, &book.save_to_buffer().unwrap(), false, "en").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(j.to_string().contains("more than 50 rows"), "{j}");

    let mut book = rust_xlsxwriter::Workbook::new();
    book.add_worksheet().write(0, 0, "secret").unwrap();
    let (status, j) = import(&router, &book.save_to_buffer().unwrap(), false, "en").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(j.to_string().contains("Unknown column: secret") || j.to_string().contains("no data rows"), "{j}");
}

#[tokio::test]
async fn broken_files_and_zip_bombs_are_refused() {
    let router = app(Parts::seeded(1));
    let mut book = rust_xlsxwriter::Workbook::new();
    book.add_worksheet().write(0, 0, "code").unwrap();
    let good = book.save_to_buffer().unwrap();
    for bad in [b"not a workbook at all".to_vec(), good[..good.len() / 2].to_vec(), Vec::new()] {
        let (status, j) = import(&router, &bad, false, "en").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{j}");
        assert!(j.to_string().contains("could not be read as an Excel workbook"), "{j}");
    }
    let mut bomb = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bomb));
        zip.start_file("xl/sharedStrings.xml", zip::write::SimpleFileOptions::default()).unwrap();
        let zeros = vec![b' '; 1024 * 1024];
        for _ in 0..80 {
            zip.write_all(&zeros).unwrap();
        }
        zip.finish().unwrap();
    }
    assert!(bomb.len() < 1024 * 1024);
    let (status, j) = import(&router, &bomb, false, "en").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(j.to_string().contains("too large"), "{j}");
}

#[tokio::test]
async fn xlsx_exports_follow_the_grid_permission_and_format_names() {
    let router = app(Parts::seeded(2));
    let page = router
        .clone()
        .oneshot(Request::get("/parts").header("x-user", "1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(page.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(html.contains(r#"data-zeyn-exportable="csv xlsx"#));
    assert!(html.contains(".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
    let req = Request::get("/_zeyn/grid/export?screen=parts&node=0&format=pdf")
        .header("x-user", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(router.clone().oneshot(req).await.unwrap().status(), StatusCode::NOT_FOUND);
    let req = Request::get("/_zeyn/grid/export?screen=parts&node=0&format=xlsx").body(Body::empty()).unwrap();
    assert_ne!(router.clone().oneshot(req).await.unwrap().status(), StatusCode::OK);
}
