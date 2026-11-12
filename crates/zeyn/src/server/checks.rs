//! Render checks of debug builds: after a screen renders, its HTML is scanned for mistakes the
//! type system cannot see, and each one is logged once per screen with what is wrong, why it
//! matters and how to fix it. Release builds skip all of it.

use std::collections::HashSet;
use std::sync::Mutex;

/// Problems in a screen's HTML: a heading that skips a level going down, a form that cannot be
/// submitted, a link inside a link. Each message says what, why and how to fix it.
///
/// ```
/// let problems = zeyn::server::render_checks("<h2>A</h2><h4>B</h4><a href=\"/x\"><a href=\"/y\">y</a></a>");
/// assert!(problems[0].contains("<h4> follows <h2>"));
/// assert!(problems[1].contains("link inside a link"));
/// ```
pub fn render_checks(html: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut last_heading: Option<u8> = None;
    let mut links = 0usize;
    let mut forms: Vec<(bool, bool)> = Vec::new();
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('>') else {
            break;
        };
        let tag = &rest[..close];
        rest = &rest[close + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                let level = name.as_bytes()[1] - b'0';
                if let Some(prev) = last_heading
                    && level > prev + 1
                {
                    problems.push(format!(
                        "<h{level}> follows <h{prev}>: a heading skips a level going down.\n  = why: screen reader users move by headings and a gap reads as missing content\n  = help: use <h{}> here (`Heading::h2` and `Heading::h3` pick the level), or put the part under a heading of the level between",
                        prev + 1
                    ));
                }
                last_heading = Some(level);
            }
            ("a", false) => {
                if links > 0 {
                    problems.push("a link inside a link: browsers split the outer link and the inner one may not work.\n  = why: nested interactive elements are invalid HTML and confuse keyboard and screen reader users\n  = help: put the inner link next to the outer one, or make the card a block with one link".into());
                }
                links += 1;
            }
            ("a", true) => links = links.saturating_sub(1),
            ("form", false) => {
                let framework =
                    tag.contains("data-zeyn-submit") || tag.contains("data-zeyn-drives");
                forms.push((framework, false));
            }
            ("button", false) | ("input", false) => {
                let submit = tag.contains(r#"type="submit""#)
                    || (name == "button" && !tag.contains("type="));
                if submit && let Some(f) = forms.last_mut() {
                    f.1 = true;
                }
            }
            ("form", true) => {
                if let Some((framework, submit)) = forms.pop()
                    && !framework
                    && !submit
                {
                    problems.push("a form without a submit button: Enter does nothing and the form cannot be sent.\n  = why: users of keyboards and assistive technology submit with Enter or a button they can find\n  = help: build the form with `Form::of(..).submit(label, action)`, or add a `<button type=\"submit\">`".into());
                }
            }
            _ => {}
        }
    }
    problems
}

/// Logs the render problems of a screen once per screen and problem, in debug builds.
pub(super) fn log(screen: &str, html: &str) {
    if !cfg!(debug_assertions) {
        return;
    }
    static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    for problem in render_checks(html) {
        let key = format!("{screen}\u{1f}{problem}");
        let first = SEEN
            .lock()
            .map(|mut s| {
                let set = s.get_or_insert_with(HashSet::new);
                set.len() < 10_000 && set.insert(key)
            })
            .unwrap_or(false);
        if first {
            tracing::warn!("render check, screen `{screen}`: {problem}");
        }
    }
}
