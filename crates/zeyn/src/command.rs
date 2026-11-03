//! Commands: things a user runs from the command palette, a menu bar or a keyboard shortcut.
//!
//! A command is declared once in Rust with its label, its shortcut and what it does. Register it
//! on a feature with [`crate::Feature::command`] so the palette lists it and its shortcut works on
//! every page, and place the same command in a [`crate::ui::MenuBar`]. Commands the user may not
//! run never reach the HTML.
//!
//! ```
//! use zeyn::prelude::*;
//!
//! #[params]
//! pub struct OrderParams {
//!     pub id: Option<i64>,
//! }
//!
//! #[screen(permission = "sales.order.write")]
//! pub fn order_form(p: OrderParams) -> Screen {
//!     Screen::window("Order")
//! }
//!
//! fn new_order() -> Command {
//!     Command::open("sales.new_order", "New order", order_form, OrderParams { id: None }).shortcut("Ctrl+Alt+N")
//! }
//!
//! let feature = Feature::new("sales").screen(order_form).command(new_order());
//! assert!(feature.check().is_ok());
//! ```
//!
//! Params of the wrong type do not compile:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[params]
//! pub struct OrderParams {
//!     pub id: Option<i64>,
//! }
//!
//! #[screen(permission = "sales.order.write")]
//! pub fn order_form(p: OrderParams) -> Screen {
//!     Screen::window("Order")
//! }
//!
//! fn new_order() -> Command {
//!     Command::open("sales.new_order", "New order", order_form, 7)
//! }
//! ```
//!
//! An action command whose action does not return `zeyn::Result<T>` does not compile:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "sales.order.write")]
//! pub async fn count_orders(_: ()) -> usize { 0 }
//!
//! fn count() -> Command {
//!     Command::action("sales.count", "Count orders", count_orders, ())
//! }
//! ```

use crate::action::{Access, Action, ActionMeta};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Effect, Icon, RowCall, ScreenDef, Window};
use serde_json::{Value as Json, json};
use std::sync::Arc;

/// The default shortcut of the command palette. It is free in every common browser; apps can
/// choose another with `App::palette_shortcut`.
pub const PALETTE_SHORTCUT: &str = "Alt+Shift+P";

/// Shortcuts browsers keep for themselves and never pass to a page: new window, new tab, close
/// tab, switch tab, quit. A command or button with one of them would never run.
pub const BROWSER_RESERVED: &[&str] = &[
    "Control+N",
    "Control+Shift+N",
    "Control+T",
    "Control+Shift+T",
    "Control+W",
    "Control+Shift+W",
    "Control+Tab",
    "Control+Shift+Tab",
    "Alt+F4",
    "Meta+N",
    "Meta+Shift+N",
    "Meta+T",
    "Meta+Shift+T",
    "Meta+W",
    "Meta+Shift+W",
    "Meta+Q",
];

/// Shortcuts a page can take over, but that people expect to do their browser's or the text
/// field's job: print, save, find, address bar, history, downloads, reload, zoom, developer
/// tools, copy and paste. Zeyn never binds them unless a command or button opts in with
/// `override_browser()`.
pub const BROWSER_DEFAULTS: &[&str] = &[
    "Control+A",
    "Control+C",
    "Control+D",
    "Control+E",
    "Control+F",
    "Control+G",
    "Control+H",
    "Control+J",
    "Control+K",
    "Control+L",
    "Control+O",
    "Control+P",
    "Control+R",
    "Control+S",
    "Control+U",
    "Control+V",
    "Control+X",
    "Control+Y",
    "Control+Z",
    "Control+Shift+A",
    "Control+Shift+B",
    "Control+Shift+C",
    "Control+Shift+Delete",
    "Control+Shift+E",
    "Control+Shift+G",
    "Control+Shift+I",
    "Control+Shift+J",
    "Control+Shift+K",
    "Control+Shift+M",
    "Control+Shift+O",
    "Control+Shift+P",
    "Control+Shift+R",
    "Control+Shift+Z",
    "Control+=",
    "Control+-",
    "Control+0",
    "Control+1",
    "Control+2",
    "Control+3",
    "Control+4",
    "Control+5",
    "Control+6",
    "Control+7",
    "Control+8",
    "Control+9",
    "Alt+D",
    "Alt+Home",
    "Alt+ArrowLeft",
    "Alt+ArrowRight",
    "F1",
    "F3",
    "F5",
    "F6",
    "F7",
    "F10",
    "F11",
    "F12",
    "Shift+F5",
    "Control+F5",
];

/// How a shortcut collides with the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserShortcut {
    /// The browser never passes it to the page.
    Reserved,
    /// The page can take it over, but only on purpose.
    Default,
}

/// Whether a normalized shortcut (see [`normalize_shortcut`]) belongs to the browser.
///
/// ```
/// use zeyn::{BrowserShortcut, browser_shortcut};
/// assert_eq!(browser_shortcut("Control+N"), Some(BrowserShortcut::Reserved));
/// assert_eq!(browser_shortcut("Control+P"), Some(BrowserShortcut::Default));
/// assert_eq!(browser_shortcut("Alt+Shift+P"), None);
/// ```
pub fn browser_shortcut(keys: &str) -> Option<BrowserShortcut> {
    if BROWSER_RESERVED.contains(&keys) {
        Some(BrowserShortcut::Reserved)
    } else if BROWSER_DEFAULTS.contains(&keys) {
        Some(BrowserShortcut::Default)
    } else {
        None
    }
}

/// The shortcut to bind, or `None` with the reason in the log when the browser keeps it.
pub(crate) fn usable_shortcut(raw: &str, override_browser: bool) -> Option<String> {
    let keys = normalize_shortcut(raw)?;
    match browser_shortcut(&keys) {
        Some(BrowserShortcut::Reserved) => {
            tracing::warn!(shortcut = %keys, "the browser keeps this shortcut and never passes it to the page; it is not bound");
            None
        }
        Some(BrowserShortcut::Default) if !override_browser => {
            tracing::warn!(shortcut = %keys, "this shortcut belongs to the browser; call override_browser() to take it over");
            None
        }
        _ => Some(keys),
    }
}

type Run = Arc<dyn Fn(Ctx) -> BoxFuture<crate::action::Result<()>> + Send + Sync>;

#[derive(Clone)]
enum Kind {
    Effect(Effect),
    Action { meta: ActionMeta, run: Run },
}

/// A command the user can run from the palette, a menu bar or its keyboard shortcut.
///
/// `id` names the command in the HTML and in `POST /_zeyn/command`. Use lower case letters,
/// digits, dots, dashes and underscores, such as `sales.new_order`. It must be unique in the app.
#[derive(Clone)]
pub struct Command {
    id: &'static str,
    label: Text,
    shortcut: Option<String>,
    override_browser: bool,
    icon: Option<Icon>,
    access: Access,
    kind: Kind,
}

impl Command {
    /// Opens a screen as a window. Only users who may open the screen see the command.
    pub fn open<S: ScreenDef>(
        id: &'static str,
        label: impl Into<Text>,
        screen: S,
        params: S::Params,
    ) -> Self {
        Self::with(
            id,
            label.into(),
            S::META.access,
            Kind::Effect(Window::open(screen, params)),
        )
    }

    /// Goes to a screen. Only users who may open the screen see the command.
    pub fn navigate<S: ScreenDef>(
        id: &'static str,
        label: impl Into<Text>,
        _screen: S,
        params: S::Params,
    ) -> Self {
        let params = serde_json::to_value(&params).expect("screen params must serialize to JSON");
        Self::with(
            id,
            label.into(),
            S::META.access,
            Kind::Effect(Effect::Navigate {
                screen: S::META.name,
                params,
            }),
        )
    }

    /// Runs an effect in the browser, such as a toast. Every signed in user of the feature sees it.
    pub fn effect(id: &'static str, label: impl Into<Text>, effect: Effect) -> Self {
        Self::with(id, label.into(), Access::Public, Kind::Effect(effect))
    }

    /// Runs `action` with `input` on the server. The input never travels to the browser: the
    /// browser names the command and the server runs it after checking the action's permission,
    /// the feature's guards and the interceptors.
    pub fn action<A>(id: &'static str, label: impl Into<Text>, _action: A, input: A::Input) -> Self
    where
        A: Action,
        A::Input: Clone + Sync,
        A::Output: Outcome,
    {
        let run: Run = Arc::new(move |ctx| {
            let input = input.clone();
            Box::pin(async move {
                let output = A::call(ctx, input).await?;
                output.failure().map_or(Ok(()), Err)
            })
        });
        Self::with(
            id,
            label.into(),
            A::META.access,
            Kind::Action { meta: A::META, run },
        )
    }

    fn with(id: &'static str, label: Text, access: Access, kind: Kind) -> Self {
        Self {
            id,
            label,
            shortcut: None,
            override_browser: false,
            icon: None,
            access,
            kind,
        }
    }

    /// Keyboard shortcut such as `"Ctrl+Shift+O"` or `"Alt+F2"`. `App::check` reports shortcuts
    /// that cannot be read, that two commands share or that the palette keeps for itself.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Lets the shortcut take over one the browser uses (see [`BROWSER_DEFAULTS`]), such as
    /// `Ctrl+P`. Shortcuts the browser never passes on ([`BROWSER_RESERVED`]) stay refused.
    pub fn override_browser(mut self) -> Self {
        self.override_browser = true;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn label(&self) -> &Text {
        &self.label
    }

    pub fn icon_of(&self) -> Option<Icon> {
        self.icon
    }

    /// The shortcut as declared.
    pub fn keys(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Who may run the command: the screen's access for `open` and `navigate`, the action's for
    /// `action`.
    pub fn access(&self) -> Access {
        self.access
    }

    /// The call of an action command, ready to run; `None` for effect commands.
    pub fn call(&self) -> Option<RowCall> {
        match &self.kind {
            Kind::Action { meta, run } => {
                let run = run.clone();
                Some(RowCall {
                    meta: *meta,
                    run: Box::new(move |ctx| run(ctx)),
                })
            }
            Kind::Effect(_) => None,
        }
    }

    /// Attributes that make an element run the command: `data-zeyn-effect` or
    /// `data-zeyn-command-run`, and `aria-keyshortcuts` with the normalized shortcut.
    pub(crate) fn attrs(&self, r: &Render) -> String {
        let mut out = format!(r#" data-zeyn-command="{}""#, escape(self.id));
        match &self.kind {
            Kind::Effect(e) => {
                out.push_str(&format!(r#" data-zeyn-effect="{}""#, attr(&e.to_json(r))))
            }
            Kind::Action { .. } => {
                out.push_str(&format!(r#" data-zeyn-command-run="{}""#, escape(self.id)))
            }
        }
        if let Some(keys) = self
            .shortcut
            .as_deref()
            .and_then(|s| usable_shortcut(s, self.override_browser))
        {
            out.push_str(&format!(r#" aria-keyshortcuts="{}""#, escape(&keys)));
        }
        out
    }

    /// The shortcut as shown to users, such as `Ctrl+Shift+O`.
    pub(crate) fn hint(&self) -> Option<String> {
        self.shortcut
            .as_deref()
            .and_then(normalize_shortcut)
            .map(|k| k.replace("Control", "Ctrl"))
    }

    /// A hidden button that runs the command, placed in the page shell for the palette and for
    /// the shortcut.
    pub(crate) fn button(&self, r: &Render) -> String {
        let hint = self
            .hint()
            .map(|h| format!(r#" data-hint="{}""#, escape(&h)))
            .unwrap_or_default();
        let icon = self
            .icon
            .and_then(|i| serde_json::to_value(i).ok())
            .and_then(|v| {
                v.as_str()
                    .map(|s| format!(r#" data-icon="{s}""#))
            })
            .unwrap_or_default();
        format!(
            r#"<button type="button" tabindex="-1"{}{hint}{icon}>{}</button>"#,
            self.attrs(r),
            r.text(&self.label)
        )
    }

    pub(crate) fn schema(&self) -> Json {
        json!({
            "id": self.id,
            "label": self.label,
            "shortcut": self.shortcut,
            "access": self.access,
            "kind": match &self.kind {
                Kind::Effect(e) => json!({ "effect": e }),
                Kind::Action { meta, .. } => json!({ "action": meta.name }),
            },
        })
    }
}

/// Reads a shortcut such as `"ctrl+shift+p"` into the form `aria-keyshortcuts` uses:
/// `Control+Shift+P`. Modifiers come in the order Control, Alt, Shift, Meta. `None` when it
/// cannot be read.
///
/// ```
/// use zeyn::normalize_shortcut;
/// assert_eq!(normalize_shortcut("ctrl+shift+p").as_deref(), Some("Control+Shift+P"));
/// assert_eq!(normalize_shortcut("Shift + Alt + f2").as_deref(), Some("Alt+Shift+F2"));
/// assert_eq!(normalize_shortcut("Cmd+K").as_deref(), Some("Meta+K"));
/// assert_eq!(normalize_shortcut("Ctrl+Ctrl+S"), None);
/// assert_eq!(normalize_shortcut("Ctrl+"), None);
/// assert_eq!(normalize_shortcut("Hyper+S"), None);
/// ```
pub fn normalize_shortcut(keys: &str) -> Option<String> {
    let parts: Vec<&str> = keys.split('+').map(str::trim).collect();
    let (key, mods) = parts.split_last()?;
    let mut flags = [false; 4];
    for m in mods {
        let i = match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => 0,
            "alt" | "option" => 1,
            "shift" => 2,
            "meta" | "cmd" | "command" | "super" => 3,
            _ => return None,
        };
        if flags[i] {
            return None;
        }
        flags[i] = true;
    }
    let key = key_name(key)?;
    let mut out: Vec<&str> = ["Control", "Alt", "Shift", "Meta"]
        .iter()
        .zip(flags)
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect();
    out.push(&key);
    Some(out.join("+"))
}

const NAMED_KEYS: [&str; 14] = [
    "Enter",
    "Escape",
    "Delete",
    "Backspace",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Tab",
];

fn key_name(key: &str) -> Option<String> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_graphic() => return Some(c.to_ascii_uppercase().to_string()),
        (None, _) => return None,
        _ => {}
    }
    if let Some(n) = key
        .strip_prefix(['F', 'f'])
        .and_then(|n| n.parse::<u8>().ok())
    {
        return (1..=24)
            .contains(&n)
            .then(|| format!("F{n}"));
    }
    NAMED_KEYS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(key))
        .map(|k| k.to_string())
}

/// Whether a normalized shortcut is a character key with no Control, Alt or Meta. WCAG 2.2
/// (2.1.4) does not allow those, because they fire while people who use speech input talk.
fn character_only(keys: &str) -> bool {
    let parts: Vec<&str> = keys.split('+').collect();
    let key = parts.last().copied().unwrap_or("");
    key.chars().count() == 1
        && parts[..parts.len() - 1]
            .iter()
            .all(|m| *m == "Shift")
}

/// Problems of commands that belong together: ids that are not usable, ids or shortcuts used
/// twice, shortcuts that cannot be read, single character shortcuts and shortcuts the palette
/// keeps for itself. `owner` names where each command was registered, for the message.
pub(crate) fn problems(
    commands: &[(&str, &Command)],
    check_each: bool,
    palette: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: Vec<(String, &str, &str)> = Vec::new();
    for (i, (owner, c)) in commands.iter().enumerate() {
        if check_each
            && (c.id.is_empty()
                || !c
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)))
        {
            out.push(format!(
                    "command `{}` has an id that is not usable in HTML\n  = help: use lower case letters, digits, dots, dashes and underscores, such as `sales.new_order`",
                    c.id
                ));
        }
        if let Some((other, _)) = commands[..i]
            .iter()
            .find(|(o, d)| d.id == c.id && (check_each || o != owner))
        {
            out.push(format!("command id `{}` is registered twice (in `{other}` and `{owner}`)\n  = help: give each command its own id", c.id));
        }
        let Some(raw) = c.shortcut.as_deref() else {
            continue;
        };
        let Some(keys) = normalize_shortcut(raw) else {
            if check_each {
                out.push(format!(
                    "command `{}` has the shortcut `{raw}`, which cannot be read\n  = help: write modifiers and one key joined by `+`, such as `Ctrl+Shift+O` or `Alt+F2`",
                    c.id
                ));
            }
            continue;
        };
        if check_each {
            if keys == palette {
                out.push(format!(
                    "command `{}` uses `{keys}`, which opens the command palette\n  = help: pick another shortcut; the palette lists the command anyway",
                    c.id
                ));
            }
            match browser_shortcut(&keys) {
                Some(BrowserShortcut::Reserved) => out.push(format!(
                    "command `{}` uses `{keys}`, which the browser keeps and never passes to the page\n  = help: pick a free shortcut such as `Alt+Shift+{}`",
                    c.id,
                    keys.rsplit('+').next().unwrap_or("")
                )),
                Some(BrowserShortcut::Default) if !c.override_browser => out.push(format!(
                    "command `{}` uses `{keys}`, which the browser uses (print, find, reload and the like)\n  = help: pick a free shortcut such as `Alt+Shift+{}`, or call `.override_browser()` to take it over on purpose",
                    c.id,
                    keys.rsplit('+').next().unwrap_or("")
                )),
                _ => {}
            }
            if character_only(&keys) {
                out.push(format!(
                    "command `{}` uses `{keys}`, a single character key; WCAG 2.2 (2.1.4) does not allow it\n  = help: add Ctrl or Alt, such as `Ctrl+{}`",
                    c.id,
                    keys.rsplit('+').next().unwrap_or("")
                ));
            }
        }
        if let Some((_, other, other_owner)) = seen
            .iter()
            .find(|(k, _, o)| *k == keys && (check_each || o != owner))
        {
            out.push(format!(
                "commands `{other}` and `{}` both use the shortcut `{keys}` (in `{other_owner}` and `{owner}`)\n  = help: give one of them another shortcut",
                c.id
            ));
        }
        seen.push((keys, c.id, owner));
    }
    out
}

/// Longest palette search query, in characters. Longer queries are cut.
pub const MAX_SEARCH_QUERY: usize = 100;

/// Most results one palette search shows, over all features.
pub const MAX_SEARCH_HITS: usize = 20;

/// One result of a palette search: what it shows and the app path it opens, such as
/// `/guides/theming#density`. The path is relative to the app's base path.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub label: String,
    /// Shown next to the label, such as the page a section belongs to.
    pub group: String,
    pub url: String,
}

impl SearchHit {
    pub fn new(label: impl Into<String>, group: impl Into<String>, url: impl Into<String>) -> Self {
        SearchHit {
            label: label.into(),
            group: group.into(),
            url: url.into(),
        }
    }

    /// Whether the path stays in the app: it starts with one `/` and has no backslash or control
    /// character. Hits that do not are dropped before they reach the browser.
    pub fn is_local(&self) -> bool {
        self.url.starts_with('/')
            && !self.url.starts_with("//")
            && !self
                .url
                .chars()
                .any(|c| c == '\\' || c.is_control())
    }
}

type Find =
    Arc<dyn Fn(Ctx, String) -> BoxFuture<crate::action::Result<Vec<SearchHit>>> + Send + Sync>;

/// A server search the command palette runs as the user types, registered with
/// [`crate::Feature::search`]. The action takes the query and returns hits.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::SearchHit;
///
/// #[action(public)]
/// pub async fn search_help(query: String) -> Result<Vec<SearchHit>> {
///     Ok(vec![SearchHit::new(format!("Help for {query}"), "Help", "/help")])
/// }
///
/// let feature = Feature::new("help").search(search_help);
/// assert_eq!(feature.searches().len(), 1);
/// ```
#[derive(Clone)]
pub struct Search {
    meta: ActionMeta,
    find: Find,
}

impl Search {
    pub fn of<A>(_action: A) -> Self
    where
        A: Action<Input = String, Output = crate::action::Result<Vec<SearchHit>>>,
    {
        let find: Find = Arc::new(|ctx, query| Box::pin(async move { A::call(ctx, query).await? }));
        Search {
            meta: A::META,
            find,
        }
    }

    pub fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    /// Runs the search. Access is checked by the caller.
    pub fn run(&self, ctx: Ctx, query: String) -> BoxFuture<crate::action::Result<Vec<SearchHit>>> {
        (self.find)(ctx, query)
    }
}
