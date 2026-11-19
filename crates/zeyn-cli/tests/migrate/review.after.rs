use zeyn::MenuGroup;

fn groups() -> Vec<MenuGroup> { Vec::new() }

fn request() -> ReorderRequest<u64> {
    ReorderRequest { key: 1, after: None, before: None }
}

fn links() -> String {
    format!("<a href=\"{}\">Out</a>", "/_zeyn/account/signout")
}
