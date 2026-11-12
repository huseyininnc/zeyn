//! Problems that `App::check`, `App::check_page` and `zeyn check` report.

use std::fmt;

/// Where the documentation site lives; [`Problem::docs`] is a path under it.
pub const DOCS_URL: &str = "https://docs.zeynsoft.com/";

/// One problem of an app's setup, written for an AI agent (or a person) to act on: what is
/// wrong, why it matters, how to fix it and where the documentation explains it.
///
/// `code` is stable across releases and names one kind of problem only, so tools can match on
/// it. Codes are grouped by area:
///
/// | Codes | Area |
/// |---|---|
/// | `Z01xx` | the app, features, routes, screens, services, themes and preferences |
/// | `Z02xx` | security: sign in, accounts, CSP and IP ranges |
/// | `Z03xx` | public pages: sitemap, site URL and the site shell |
/// | `Z04xx` | menus, commands and keyboard shortcuts |
/// | `Z05xx` | data: media sources, jobs and workflows |
/// | `Z06xx` | page checks of components (`App::check_page`) |
///
/// The text form (its `Display`) keeps the first line as the summary and adds one line each for
/// the reason, the fix, the documentation and the code:
///
/// ```
/// use zeyn::Problem;
///
/// let p = Problem::new("Z0101", "route `/a` is used by screens `x` and `y`")
///     .why("a request to the route can only open one screen")
///     .fix("give one of the screens another route")
///     .docs("language/screens#params-in-the-path");
/// let text = p.to_string();
/// assert!(text.starts_with("route `/a` is used by screens `x` and `y`\n  = why: "));
/// assert!(text.ends_with("\n  = code: Z0101"));
/// assert_eq!(zeyn::problem_json(&text), p.json());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Stable code such as `Z0101`.
    pub code: &'static str,
    /// What is wrong, on one line.
    pub summary: String,
    /// Why it matters.
    pub why: String,
    /// What to change.
    pub fix: String,
    /// A path on the documentation site, such as `language/screens#access`.
    pub docs: &'static str,
}

impl Problem {
    /// A problem with its code and summary; add the reason, the fix and the docs path.
    pub fn new(code: &'static str, summary: impl Into<String>) -> Self {
        Self {
            code,
            summary: summary.into(),
            why: String::new(),
            fix: String::new(),
            docs: "",
        }
    }

    /// Why the problem matters.
    pub fn why(mut self, why: impl Into<String>) -> Self {
        self.why = why.into();
        self
    }

    /// What to change to fix it.
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = fix.into();
        self
    }

    /// The documentation page, a path under [`DOCS_URL`] such as `guides/security#limits`.
    pub fn docs(mut self, docs: &'static str) -> Self {
        self.docs = docs;
        self
    }

    /// The same problem with `prefix` put before its summary, such as "feature `shop`: ".
    pub fn prefixed(mut self, prefix: &str) -> Self {
        self.summary.insert_str(0, prefix);
        self
    }

    /// The problem as JSON: `code`, `summary`, `why`, `fix` and `docs` (the full URL).
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code,
            "summary": self.summary,
            "why": self.why,
            "fix": self.fix,
            "docs": format!("{DOCS_URL}{}", self.docs),
        })
    }
}

/// The JSON form of a problem's text form (see [`Problem::json`]), for problems that are kept as
/// strings, such as those of `App::check`. A text without the extra lines keeps an empty code.
pub fn problem_json(text: &str) -> serde_json::Value {
    let mut parts = text.split(
        "
  = ",
    );
    let summary = parts.next().unwrap_or_default();
    let mut out = serde_json::json!({
        "code": "", "summary": summary, "why": "", "fix": "", "docs": "",
    });
    for part in parts {
        for (prefix, key) in [
            ("why: ", "why"),
            ("help: ", "fix"),
            ("docs: ", "docs"),
            ("code: ", "code"),
        ] {
            if let Some(v) = part.strip_prefix(prefix) {
                out[key] = v.into();
            }
        }
    }
    out
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n  = why: {}\n  = help: {}\n  = docs: {DOCS_URL}{}\n  = code: {}",
            self.summary, self.why, self.fix, self.docs, self.code
        )
    }
}

impl From<Problem> for String {
    fn from(p: Problem) -> String {
        p.to_string()
    }
}
