use zeyn::MenuGroup;

fn groups() -> Vec<MenuGroup> { Vec::new() }

fn request() -> ReorderRequest<u64> {
    ReorderRequest { key: 1, after: None, before: None }
}

fn links() -> String {
    format!("<a href=\"{}\">Out</a>", "/_zeyn/account/signout")
}

fn kind(k: CallKind) -> bool {
    matches!(k, CallKind::Import)
}

struct Limits;

impl RateStore for Limits {}

impl DocStore for Docs {}

fn check_page(page: &mut PageCheck) {}

fn shared(s: Arc<dyn RateStore>) {}

fn preview() -> CallKind {
    CallKind::ImportPreview
}
