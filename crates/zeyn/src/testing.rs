//! Test helpers for screens and actions: [`TestApp`] drives an app in memory, through the same
//! router, checks and access rules as a served app, without opening a port.
//!
//! It plays the part Angular's `TestBed` or Flutter's widget tester plays: build the app once,
//! sign a user in, open a screen, submit a form, page a grid or call an action, then read the
//! status, the HTML or the JSON. CSRF tokens are sent for you. A screen on its own needs no
//! helper: `my_screen::run()` builds a screen without arguments, and `screen.html(&my_screen::META)`
//! and `screen.schema(&my_screen::META)` render and describe it.
//!
//! ```
//! use zeyn::prelude::*;
//! use zeyn::testing::TestApp;
//!
//! #[screen(route = "/hello", permission = "hello.read")]
//! pub fn hello() -> Screen {
//!     Screen::new("Hello").body(Paragraph::new("Hi"))
//! }
//!
//! fn app() -> App {
//!     App::new("Test").feature(Feature::new("hello").screen(hello))
//! }
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let anonymous = TestApp::new(app());
//! assert_eq!(anonymous.get("/hello").await.status, 403);
//!
//! let ayse = TestApp::signed_in(app(), Identity::user("ayse").with_permissions(["hello.read"]));
//! let page = ayse.get("/hello").await;
//! assert_eq!(page.status, 200);
//! assert!(page.body.contains("Hi"));
//! # });
//! ```

use crate::ctx::Identity;
use crate::server::{App, Authenticator};
use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, header};
use serde_json::Value as Json;
use tower_service::Service;

/// `json!` for the values, queries and inputs of a test, so a test needs no `serde_json` of its
/// own.
pub use serde_json::json;

/// A CSRF token every request of a [`TestApp`] carries, as the cookie and as the header.
const TOKEN: &str = "7e57000000000000000000000000000000000000000000000000000000000000";

/// The largest response body a test reads.
const BODY_LIMIT: usize = 64 * 1024 * 1024;

/// An app driven in memory for tests. See the [module documentation](self).
#[derive(Clone)]
pub struct TestApp {
    router: axum::Router,
    api_prefix: String,
    headers: Vec<(HeaderName, HeaderValue)>,
}

/// What a request to a [`TestApp`] returned.
#[derive(Debug)]
pub struct TestResponse {
    /// The HTTP status, such as `200`.
    pub status: u16,
    pub headers: HeaderMap,
    /// The body as text.
    pub body: String,
}

impl TestResponse {
    /// The body as JSON, or `Json::Null` when it is not JSON.
    pub fn json(&self) -> Json {
        serde_json::from_str(&self.body).unwrap_or(Json::Null)
    }

    /// The kinds of the effects a form or an action returned, in order, such as
    /// `["toast", "refresh"]`.
    pub fn effects(&self) -> Vec<String> {
        self.json()["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e["effect"].as_str())
            .map(str::to_string)
            .collect()
    }
}

/// Signs every request in as one identity.
struct SignedIn(Identity);

impl Authenticator for SignedIn {
    fn authenticate(&self, _: &HeaderMap) -> Option<Identity> {
        Some(self.0.clone())
    }
}

impl TestApp {
    /// The app as it is configured, with its own sign in. Panics with the app's problems when
    /// `App::check` finds any, like `App::serve` refuses to start.
    #[track_caller]
    pub fn new(app: App) -> Self {
        let api_prefix = app.api_path().to_string();
        match app.router() {
            Ok(router) => Self {
                router,
                api_prefix,
                headers: Vec::new(),
            },
            Err(problems) => panic!("the app has problems:\n{}", problems.join("\n")),
        }
    }

    /// The app with every request signed in as `identity`, in place of its own sign in, so a
    /// test can try what each permission allows.
    #[track_caller]
    pub fn signed_in(app: App, identity: Identity) -> Self {
        Self::new(app.auth(SignedIn(identity)))
    }

    /// Adds a header to every request, such as `accept-language` to see the app in another
    /// language. `name` is lower case. Panics on a value HTTP does not allow.
    #[track_caller]
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        let value =
            HeaderValue::from_str(value).expect("a header value without control characters");
        self.headers
            .push((HeaderName::from_static(name), value));
        self
    }

    /// Sends a request with the headers of [`TestApp::header`] and reads the whole response.
    pub async fn send(&self, mut request: Request<Body>) -> TestResponse {
        for (name, value) in &self.headers {
            request
                .headers_mut()
                .insert(name.clone(), value.clone());
        }
        let mut router = self.router.clone();
        let response = match router.call(request).await {
            Ok(r) => r,
            Err(never) => match never {},
        };
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), BODY_LIMIT)
            .await
            .unwrap_or_default();
        TestResponse {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    /// `GET path`: a page, a file or an endpoint.
    pub async fn get(&self, path: &str) -> TestResponse {
        let request = Request::get(path)
            .body(Body::empty())
            .expect("a request path such as /notes");
        self.send(request).await
    }

    /// `POST path` with a JSON body and a valid CSRF token.
    pub async fn post(&self, path: &str, body: Json) -> TestResponse {
        let request = Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
            .header("x-zeyn-csrf", TOKEN)
            .body(Body::from(body.to_string()))
            .expect("a request path such as /notes");
        self.send(request).await
    }

    /// Submits the form at `node` (a path such as `"0"` or `"1.0"`) of the screen named `screen`,
    /// opened with `params` (`Json::Null` when it takes none), the way the browser does. A saved
    /// form answers `200` with its effects ([`TestResponse::effects`]); rule errors answer `422`
    /// with `errors`.
    pub async fn submit(
        &self,
        screen: &str,
        params: Json,
        node: &str,
        values: Json,
    ) -> TestResponse {
        let body = json!({ "screen": screen, "params": params, "node": node, "values": values });
        self.post("/_zeyn/form", body).await
    }

    /// Loads a page of the grid at `node` of the screen named `screen`, with a `DataQuery` as
    /// JSON (`json!({})` for the first page). The answer holds `rows` (HTML) and `total`.
    pub async fn grid(&self, screen: &str, params: Json, node: &str, query: Json) -> TestResponse {
        let body = json!({ "screen": screen, "params": params, "node": node, "query": query });
        self.post("/_zeyn/grid", body).await
    }

    /// Calls the action `action` of `feature` through the JSON API (`App::api` must publish it),
    /// with its input as JSON. The request carries `Authorization: Bearer test`, as API calls
    /// need one; the app's sign in (or [`TestApp::signed_in`]) decides who it is.
    pub async fn api(&self, feature: &str, action: &str, input: Json) -> TestResponse {
        let path = format!("{}/{feature}/{action}", self.api_prefix);
        let request = Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, "Bearer test")
            .body(Body::from(input.to_string()))
            .expect("a feature and an action name");
        self.send(request).await
    }
}
