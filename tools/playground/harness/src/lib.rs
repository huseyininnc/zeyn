//! The fixed half of a playground program. It is compiled once into the sandbox image, next to
//! `zeyn`, so a run compiles only the small crate around the person's screen and links it
//! against these prebuilt libraries.

use zeyn::prelude::*;
use zeyn::testing::TestApp;

/// Renders the page at `/` of `app` as an anonymous visitor, in memory, and prints the HTML.
/// Exits with status 2 when the page does not answer 200.
pub fn print_page(app: App) {
    let page = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current thread runtime")
        .block_on(TestApp::new(app).get("/"));
    if page.status != 200 {
        eprintln!("the screen answered {}", page.status);
        std::process::exit(2);
    }
    print!("{}", page.body);
}
