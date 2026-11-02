use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use zeyn::prelude::*;
use zeyn::{Access, Component, Cursor, FieldKind, FieldType, PageRequest, Render, Rule, Value};

/// A product kept in stock.
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    #[label("Product name")]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
}

/// Unit of measure.
#[choice]
pub enum Unit {
    Piece,
    #[label("Kilogram")]
    Kg,
    Liter,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> {
    Ok(Page::new(vec![sample()]).total(1))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>) -> Result<Product> {
    Ok(product.into_inner())
}

#[params]
pub struct ProductFormParams {
    pub id: Option<ProductId>,
}

impl ProductFormParams {
    pub fn new_product() -> Self {
        Self { id: None }
    }

    pub fn edit(id: ProductId) -> Self {
        Self { id: Some(id) }
    }
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new("Products")
        .toolbar(|t| {
            t.button("New product")
                .icon(Icon::Plus)
                .shortcut("Ctrl+N")
                .opens(product_form, ProductFormParams::new_product())
                .export(Export::Xlsx)
        })
        .body(product_grid())
}

fn product_grid() -> DataGrid<Product> {
    DataGrid::of(search_products)
        .mode(DataMode::Server)
        .column(Product::code)
        .width(120)
        .filter()
        .column(Product::name)
        .flex(1)
        .filter()
        .column(Product::unit)
        .column(Product::price)
        .column(Product::stock)
        .sum()
        .row_style(|p| (p.stock <= 0).then_some(RowStyle::Warning))
        .on_open(|p| Window::open(product_form, ProductFormParams::edit(p.id)))
}

#[screen(permission = "stock.product.write")]
pub fn product_form(params: ProductFormParams) -> Screen {
    let product = match params.id {
        Some(id) => Product { id, ..sample() },
        None => Product::default(),
    };
    Screen::window("Product").width(640).body(product_editor(product))
}

fn product_editor(product: Product) -> zeyn::SubmitForm<Product, save_product> {
    Form::of(product)
        .row(|r| r.field(Product::code).field(Product::unit))
        .field(Product::name)
        .row(|r| r.field(Product::price).field(Product::active))
        .submit("Save", save_product)
        .on_success(|_, ctx| {
            ctx.toast("Product saved");
            ctx.close_and_refresh();
        })
}

fn sample() -> Product {
    Product {
        id: ProductId(7),
        code: "P-001".into(),
        name: "Tea <Rize>".into(),
        unit: Unit::Kg,
        price: Money::tl(123456),
        stock: 0,
        active: true,
    }
}

fn feature() -> Feature {
    Feature::new("stock")
        .route_prefix("/stock")
        .screen(product_list)
        .screen(product_form)
        .menu(|m| m.group("Stock", Icon::Box).item("Products", product_list))
}

/// Leave request: a model that is not stored as a table row, with a rule across two fields.
#[model(validate = check_dates)]
pub struct LeaveRequest {
    #[rule(required, min_len = 10)]
    pub reason: String,
    #[rule(required, email)]
    pub manager_email: String,
    #[rule(min = 1)]
    pub starts_day: i64,
    #[rule(max = 365)]
    pub ends_day: i64,
    pub phone: Phone,
}

fn check_dates(r: &LeaveRequest, errors: &mut ValidationErrors) {
    if r.ends_day < r.starts_day {
        errors.add(LeaveRequest::ends_day, "End day must not be before the start day.");
    }
}

/// Field type defined outside Zeyn, as a plugin crate would do.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Phone(pub String);

impl FieldType for Phone {
    const KIND: FieldKind = FieldKind::Custom { editor: "phone" };

    fn to_value(&self) -> Value {
        Value::Text(self.0.clone())
    }
}

/// Component defined outside Zeyn, as a plugin crate would do.
struct StatCard {
    label: &'static str,
    value: String,
}

impl Component for StatCard {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="stat" data-testid="{}.stat">{}: {}</div>"#,
            r.scope,
            zeyn::render::escape(self.label),
            zeyn::render::escape(&self.value)
        ));
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "stat_card", "label": self.label })
    }
}

impl From<StatCard> for Node {
    fn from(c: StatCard) -> Self {
        Node::new(c)
    }
}

#[screen(route = "/dashboard", public)]
pub fn dashboard() -> Screen {
    Screen::new("Dashboard").body(StatCard { label: "Open <orders>", value: "12".into() })
}

fn leave_request() -> LeaveRequest {
    LeaveRequest {
        reason: "Family visit abroad".into(),
        manager_email: "lead@example.com".into(),
        starts_day: 10,
        ends_day: 14,
        phone: Phone("+90 555 000 00 00".into()),
    }
}

fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[test]
fn field_definitions_are_generated() {
    let code = Product::code.meta();
    assert_eq!(code.name, "code");
    assert_eq!(code.label, "Code");
    assert_eq!(code.kind, FieldKind::Text);
    assert_eq!(code.rules, &[Rule::Required, Rule::MaxLen(20), Rule::Unique]);
    assert_eq!(Product::name.meta().label, "Product name");
    assert!(Product::id.meta().key);
    assert!(Product::stock.meta().readonly);
    assert_eq!(<Product as Entity>::TABLE, "products");
    assert_eq!(<Product as Model>::NAME, "Product");
    assert_eq!(Product::fields().len(), 7);
    assert_eq!(sample().value_at(1), Some(Value::Text("P-001".into())));
    assert_eq!(sample().value_at(99), None);
}

#[test]
fn field_rules_run() {
    let ok = sample();
    assert!(Valid::new(ok.clone()).is_ok());

    let bad = Product { code: "  ".into(), name: "x".repeat(201), price: Money::tl(-1), ..ok };
    let errors = Valid::new(bad).unwrap_err();
    let fields: Vec<&str> = errors.errors.iter().map(|e| e.field).collect();
    assert_eq!(fields, ["code", "name", "price"]);
    assert_eq!(errors.errors[0].message.english(), "Code is required.");
    assert_eq!(errors.errors[1].message.english(), "Product name must be at most 200 characters.");
}

#[test]
fn max_len_counts_characters_not_bytes() {
    let product = Product { code: "ğ".repeat(20), ..sample() };
    assert!(Valid::new(product).is_ok());
}

#[test]
fn deserializing_valid_runs_validation() {
    let json = r#"{"id":1,"code":"","name":"Water","unit":"Piece","price":{"minor":100,"currency":"Try"},"stock":5,"active":true}"#;
    let result: std::result::Result<Valid<Product>, _> = serde_json::from_str(json);
    assert!(result.unwrap_err().to_string().contains("Code is required."));
}

#[test]
fn model_rules_and_cross_field_rule() {
    assert!(Valid::new(leave_request()).is_ok());

    let bad = LeaveRequest {
        reason: "short".into(),
        manager_email: "lead@@example".into(),
        starts_day: 0,
        ends_day: 400,
        ..leave_request()
    };
    let errors = Valid::new(bad).unwrap_err();
    let pairs: Vec<(&str, Rule)> = errors.errors.iter().map(|e| (e.field, e.rule)).collect();
    assert_eq!(
        pairs,
        [
            ("reason", Rule::MinLen(10)),
            ("manager_email", Rule::Email),
            ("starts_day", Rule::Min(1)),
            ("ends_day", Rule::Max(365)),
        ]
    );

    let reversed = LeaveRequest { starts_day: 20, ends_day: 5, ..leave_request() };
    let errors = Valid::new(reversed).unwrap_err();
    assert_eq!(errors.errors.len(), 1);
    assert_eq!(errors.errors[0].field, "ends_day");
    assert_eq!(errors.errors[0].rule, Rule::Custom);
}

#[test]
fn empty_optional_text_skips_min_len_and_email() {
    let r = LeaveRequest { reason: String::new(), manager_email: String::new(), ..leave_request() };
    let errors = Valid::new(r).unwrap_err();
    let rules: Vec<Rule> = errors.errors.iter().map(|e| e.rule).collect();
    assert_eq!(rules, [Rule::Required, Rule::Required]);
}

#[test]
fn auto_form_renders_every_field_with_client_rules() {
    let html = zeyn::render::escape("");
    assert!(html.is_empty());
    let mut out = String::new();
    Form::auto(leave_request()).render(&mut out, &Render::english("leave"));
    assert!(out.contains(r#"name="reason""#));
    assert!(out.contains(r#"minlength="10""#));
    assert!(out.contains(r#"type="email" id="leave-manager_email""#));
    assert!(out.contains(r#"min="1""#));
    assert!(out.contains(r#"max="365""#));
    assert!(out.contains(r#"data-zeyn-editor="phone" value="+90 555 000 00 00""#));
    assert!(out.contains(r#"aria-describedby="leave-reason-error""#));
}

#[test]
fn money_is_formatted() {
    assert_eq!(Money::tl(123456).format("tr"), "1.234,56 ₺");
    assert_eq!(Money::tl(-5).format("tr"), "-0,05 ₺");
    assert_eq!(Money::tl(100000000).format("tr"), "1.000.000,00 ₺");
    assert_eq!(Money::tl(123456).to_string(), "₺1,234.56");
    assert_eq!(Money::tl(-5).format("en-US"), "-₺0.05");
    assert_eq!(Money::tl(i64::MIN).format_plain("tr"), "-92.233.720.368.547.758,08");
    let tl = zeyn::Currency::Try;
    assert_eq!(Money::parse("1.234,5", "tr", tl), Ok(Money::tl(123450)));
    assert_eq!(Money::parse("1,234.56", "en", tl), Ok(Money::tl(123456)));
    assert_eq!(Money::parse("-0,05 ₺", "tr", tl), Ok(Money::tl(-5)));
    assert_eq!(Money::parse("12", "tr", tl), Ok(Money::tl(1200)));
    assert!(Money::parse("1,234", "tr", tl).is_err());
    assert!(Money::parse("abc", "tr", tl).is_err());
    assert!(Money::parse("", "tr", tl).is_err());
    assert!(Money::parse("99999999999999999999", "tr", tl).is_err());
}

#[test]
fn action_meta_and_run() {
    assert_eq!(save_product::META.name, "save_product");
    assert_eq!(save_product::META.access, Access::Permission("stock.product.write"));
    assert!(save_product::META.audit);
    assert!(!search_products::META.audit);

    let saved = block_on(save_product::run(Valid::new(sample()).unwrap())).unwrap();
    assert_eq!(saved.code, "P-001");
}

#[test]
fn list_screen_renders() {
    let html = product_list::run().html(&product_list::META);
    assert!(html.contains(r#"data-zeyn-screen="product_list""#));
    assert!(html.contains(r#"data-testid="product_list.toolbar.new-product""#));
    assert!(html.contains(r#"aria-keyshortcuts="Control+N""#));
    assert!(html.contains(r#"data-testid="product_list.toolbar.export-xlsx""#));
    assert!(html.contains(r#"data-zeyn-action="search_products""#));
    assert!(html.contains(r#"data-zeyn-mode="server""#));
    assert!(html.contains(
        r#"<th scope="col" data-field="code" aria-sort="none" aria-colindex="1" style="width:120px;" data-filter="text">Code</th>"#
    ));
    assert!(html.contains(r#"data-field="stock" data-agg="sum""#));
    assert!(html.contains("&quot;screen&quot;:&quot;product_form&quot;"));
}

#[test]
fn grid_rows_are_escaped_and_highlighted() {
    let rows = product_grid().rows_html(&[sample()]);
    assert!(rows.starts_with(r#"<tr class="z-row-warning" tabindex="-1" data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;"#));
    assert!(rows.contains("Tea &lt;Rize&gt;"));
    assert!(rows.contains(">Kilogram<"));
    assert!(rows.contains(r#"<td class="z-num">₺1,234.56</td>"#));
    assert!(!rows.contains("<Rize>"));
}

#[test]
fn opening_a_row_opens_the_form_window() {
    let effect = product_grid().open_effect(&sample()).unwrap();
    assert_eq!(effect, Effect::OpenWindow { screen: "product_form", params: serde_json::json!({ "id": 7 }) });
}

#[test]
fn form_renders() {
    let html = product_form::run(ProductFormParams::edit(ProductId(7))).html(&product_form::META);
    assert!(html.contains(r#"role="dialog""#));
    assert!(html.contains("--z-width:640px"));
    assert!(html.contains(r#"data-zeyn-submit="save_product""#));
    assert!(html.contains(r#"data-testid="product_form.field.code" aria-describedby="product_form-code-error" required aria-required="true" maxlength="20" value="P-001""#));
    assert!(html.contains(r#"<option value="Kg" selected>Kilogram</option>"#));
    assert!(html.contains(r#"value="1,234.56""#));
    assert!(html.contains(r#"type="checkbox" class="z-checkbox" id="product_form-active""#));
    assert!(html.contains(r#"value="Tea &lt;Rize&gt;""#));
}

#[test]
fn successful_save_produces_effects() {
    let form = product_editor(sample());
    let effects = form.success_effects(&Ok(sample()));
    assert_eq!(effects, vec![Effect::Toast { message: "Product saved".into() }, Effect::CloseAndRefresh]);
}

#[test]
fn screen_schema_is_generated() {
    let schema = product_form::run(ProductFormParams::new_product()).schema(&product_form::META);
    assert_eq!(schema["screen"]["name"], "product_form");
    assert_eq!(schema["screen"]["access"]["permission"], "stock.product.write");
    assert_eq!(schema["kind"], "window");
    let form = &schema["body"][0];
    assert_eq!(form["model"], "Product");
    assert_eq!(form["submit"]["action"]["name"], "save_product");
    assert_eq!(form["fields"][0]["field"]["name"], "code");
    assert_eq!(form["fields"][0]["row"], 0);
    assert_eq!(form["fields"][2]["field"]["name"], "name");
    assert_eq!(form["fields"][1]["field"]["kind"]["type"], "choice");
}

#[test]
fn third_party_component_renders_and_describes_itself() {
    let screen = dashboard::run();
    let html = screen.html(&dashboard::META);
    assert!(html.contains(r#"data-testid="dashboard.stat">Open &lt;orders&gt;: 12"#));
    assert_eq!(screen.schema(&dashboard::META)["body"][0]["type"], "stat_card");
    assert_eq!(dashboard::META.access, Access::Public);
}

#[test]
fn feature_routes_and_menu() {
    let f = feature();
    assert_eq!(f.routes(), vec![zeyn::Route { path: "/stock/products".into(), screen: "product_list" }]);
    assert_eq!(f.menu_groups()[0].items[0].screen, "product_list");
    assert!(f.check().is_ok());
}

#[test]
fn feature_check_finds_problems() {
    let f = Feature::new("stock").screen(product_list).screen(product_list);
    let problems = f.check().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("route `/products`")));
    assert!(problems.iter().any(|p| p.contains("registered twice")));

    let missing = Feature::new("stock").menu(|m| m.group("Stock", Icon::Box).item("Products", product_list));
    assert!(missing.check().unwrap_err()[0].contains("not registered"));
}

#[test]
fn slug_simplifies_turkish_letters() {
    assert_eq!(zeyn::render::slug("Yeni Ürün Ekle"), "yeni-urun-ekle");
    assert_eq!(zeyn::render::slug("İŞÇİ ĞÖÜ ı"), "isci-gou-i");
    assert_eq!(zeyn::render::slug("--Save!--"), "save");
}

#[test]
fn grids_page_by_default() {
    let html = product_list::run().html(&product_list::META);
    assert!(html.contains(r#"data-zeyn-paging="pages" data-zeyn-page-size="50""#));
    assert!(html.contains(r#"<nav class="z-pager" aria-label="Pagination" data-testid="product_list.pager">"#));
    assert!(!html.contains("load-more"));
}

#[test]
fn infinite_scroll_keeps_a_load_more_button() {
    let mut out = String::new();
    product_grid().paging(Paging::infinite(100)).render(&mut out, &Render::english("products"));
    assert!(out.contains(r#"data-zeyn-paging="infinite" data-zeyn-page-size="100""#));
    assert!(out.contains(r#"data-testid="products.grid.load-more">Load more</button>"#));
    assert!(!out.contains("z-pager"));
}

#[test]
fn page_sizes_are_bounded() {
    assert_eq!(Paging::pages(0), Paging::Pages { size: 1 });
    assert_eq!(Paging::infinite(10_000), Paging::Infinite { batch: zeyn::MAX_PAGE_SIZE });

    let greedy: DataQuery = serde_json::from_str(r#"{"page":{"type":"offset","offset":0,"limit":4000000000}}"#).unwrap();
    assert_eq!(greedy.limit(), zeyn::MAX_PAGE_SIZE);
    let empty: DataQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.limit(), zeyn::DEFAULT_PAGE_SIZE);
}

#[test]
fn in_memory_paging_by_offset_and_cursor() {
    let all: Vec<i64> = (1..=7).collect();

    let q = DataQuery { page: PageRequest::Offset { offset: 5, limit: 3 }, ..Default::default() };
    let p = Page::from_slice(&all, &q);
    assert_eq!(p.items, [6, 7]);
    assert_eq!(p.total, Some(7));
    assert_eq!(p.next, None);

    let q = DataQuery { page: PageRequest::After { cursor: None, limit: 3 }, ..Default::default() };
    let first = Page::from_slice(&all, &q);
    assert_eq!(first.items, [1, 2, 3]);
    let q = DataQuery { page: PageRequest::After { cursor: first.next.clone(), limit: 3 }, ..Default::default() };
    let second = Page::from_slice(&all, &q);
    assert_eq!(second.items, [4, 5, 6]);
    assert_eq!(second.next, Some(Cursor("6".into())));

    let q = DataQuery { page: PageRequest::Offset { offset: u64::MAX, limit: 3 }, ..Default::default() };
    assert!(Page::from_slice(&all, &q).items.is_empty());
    let q = DataQuery { page: PageRequest::After { cursor: Some(Cursor("junk".into())), limit: 2 }, ..Default::default() };
    assert_eq!(Page::from_slice(&all, &q).items, [1, 2]);
}

#[test]
fn auto_grid_has_every_field_and_filters_text() {
    let mut out = String::new();
    DataGrid::auto(search_products).render(&mut out, &Render::english("p"));
    for f in ["id", "code", "name", "unit", "price", "stock", "active"] {
        assert!(out.contains(&format!(r#"data-field="{f}""#)), "{f}");
    }
    assert!(out.contains(r#"data-field="name" aria-sort="none" aria-colindex="3" data-filter="text""#));
    assert!(!out.contains(r#"data-field="price" aria-sort="none" aria-colindex="5" data-filter"#));
}
