//! [`CodeEditor`] and [`Control::Code`](super::Control::Code): an editable code field for SQL and
//! report expressions, with highlighting, line numbers, completion and error markers.
//!
//! The real input is a plain `<textarea>`, so native editing, undo, input methods, mobile
//! keyboards and screen readers work as for any text field. `codeedit.js` lays a highlighted copy of
//! the text (`<pre>`, hidden from assistive technology) under it and keeps the two in sync line
//! by line: a keystroke highlights the lines it changed and the lines after it whose starting
//! state changed, so typing in a long text stays cheap. Without the script the field is a plain
//! text area.
//!
//! Completion and error checks call actions through `POST /_zeyn/data`, which checks the CSRF
//! token, the screen's access and the action's permission, and counts against the write limit.
//! The browser asks only after a pause in typing and cancels requests that are out of date. The
//! server refuses texts over [`MAX_CODE_QUERY_BYTES`] and caps what the actions return
//! ([`MAX_COMPLETIONS`], [`MAX_CODE_ERRORS`], [`MAX_COMPLETION_CHARS`], [`MAX_CODE_MESSAGE_CHARS`]).

use super::DataSource;
use super::code::CodeLang;
use super::layout::into_node;
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

into_node!(CodeEditor);

/// Most bytes of text a completion or check request may carry; the server refuses longer ones.
/// For a longer text the browser sends the part around the cursor for completion and does not
/// ask for a check.
pub const MAX_CODE_QUERY_BYTES: usize = 64 * 1024;

/// Most completions one request returns, whatever the action gives.
pub const MAX_COMPLETIONS: usize = 50;

/// Most characters of a completion's label, inserted text and detail; longer ones are cut.
pub const MAX_COMPLETION_CHARS: usize = 200;

/// Most error markers one check returns, whatever the action gives.
pub const MAX_CODE_ERRORS: usize = 100;

/// Most characters of an error message; longer ones are cut.
pub const MAX_CODE_MESSAGE_CHARS: usize = 500;

/// What a completion or check action receives: the text and the cursor, in characters from the
/// start. For a long text (see [`MAX_CODE_QUERY_BYTES`]) a completion gets only the part around
/// the cursor. Both come from the client: never trust them further than to suggest or check.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeQuery {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub cursor: usize,
}

impl CodeQuery {
    /// The text before the cursor; the whole text when the cursor is past its end.
    ///
    /// ```
    /// use zeyn::ui::CodeQuery;
    /// let q = CodeQuery { text: "[Ord] + 1".into(), cursor: 4 };
    /// assert_eq!(q.before(), "[Ord");
    /// ```
    pub fn before(&self) -> &str {
        let end = self
            .text
            .char_indices()
            .nth(self.cursor)
            .map_or(self.text.len(), |(i, _)| i);
        &self.text[..end]
    }

    /// The word being typed: the letters, digits, `_` and `.` right before the cursor. A
    /// [`Completion`]'s `insert` replaces exactly this word.
    ///
    /// ```
    /// use zeyn::ui::CodeQuery;
    /// let q = CodeQuery { text: "[Orders.To".into(), cursor: 10 };
    /// assert_eq!(q.word(), "Orders.To");
    /// assert_eq!(CodeQuery { text: "a + [".into(), cursor: 5 }.word(), "");
    /// ```
    pub fn word(&self) -> &str {
        let before = self.before();
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '.')
            .last()
            .map_or(before.len(), |(i, _)| i);
        &before[start..]
    }
}

/// What a completion is, shown next to its label in the user's language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionKind {
    Keyword,
    Function,
    Table,
    #[default]
    Field,
    Value,
    Snippet,
}

/// One completion: the label shown, the text that replaces the word before the cursor
/// ([`CodeQuery::word`]), its kind and an optional detail such as a type.
///
/// ```
/// use zeyn::ui::{Completion, CompletionKind};
/// let c = Completion::new(CompletionKind::Table, "Orders").insert("Orders.").detail("12 fields");
/// assert_eq!(c.insert, "Orders.");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    pub label: String,
    pub insert: String,
    pub kind: CompletionKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl Completion {
    /// A completion that inserts its label.
    pub fn new(kind: CompletionKind, label: impl Into<String>) -> Self {
        let label = label.into();
        Completion {
            insert: label.clone(),
            label,
            kind,
            detail: String::new(),
        }
    }

    /// The text to insert in place of the word before the cursor, when it is not the label.
    pub fn insert(mut self, insert: impl Into<String>) -> Self {
        self.insert = insert.into();
        self
    }

    /// A short second text, such as a type or a description.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
}

/// A problem in a code text, underlined in the editor and listed under it: `line` and `column`
/// from 1, `length` in characters (0 marks a point). The editor keeps a marker inside the text:
/// a line past the end marks the last line, and a span never runs past the end of its line.
///
/// ```
/// use zeyn::ui::CodeError;
/// let e = CodeError::new(1, 9, 6, "Unknown field");
/// assert_eq!((e.line, e.column, e.length), (1, 9, 6));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CodeError {
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub message: Text,
}

impl CodeError {
    pub fn new(line: u32, column: u32, length: u32, message: impl Into<Text>) -> Self {
        CodeError {
            line,
            column,
            length,
            message: message.into(),
        }
    }
}

/// `s` cut to at most `max` characters.
fn cut(mut s: String, max: usize) -> String {
    if let Some((i, _)) = s.char_indices().nth(max) {
        s.truncate(i);
    }
    s
}

/// Reads a request: refuses a text over [`MAX_CODE_QUERY_BYTES`] and keeps the cursor inside it.
fn read_query(input: Json) -> Result<CodeQuery> {
    let mut q: CodeQuery = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
    if q.text.len() > MAX_CODE_QUERY_BYTES {
        return Err(Error::Denied(Text::key("zeyn.ui.code.too_long")));
    }
    q.cursor = q.cursor.min(q.text.chars().count());
    Ok(q)
}

/// The completions sent to the browser: at most [`MAX_COMPLETIONS`], without empty labels, every
/// text cut to [`MAX_COMPLETION_CHARS`].
pub(crate) fn completions_json(items: Vec<Completion>) -> Json {
    let items: Vec<Completion> = items
        .into_iter()
        .filter(|c| !c.label.is_empty())
        .take(MAX_COMPLETIONS)
        .map(|c| Completion {
            label: cut(c.label, MAX_COMPLETION_CHARS),
            insert: cut(c.insert, MAX_COMPLETION_CHARS),
            kind: c.kind,
            detail: cut(c.detail, MAX_COMPLETION_CHARS),
        })
        .collect();
    serde_json::to_value(items).unwrap_or(Json::Null)
}

/// The error markers sent to the browser: at most [`MAX_CODE_ERRORS`], clamped to the lines of
/// `text`, messages in the user's language cut to [`MAX_CODE_MESSAGE_CHARS`].
pub(crate) fn errors_json(errors: Vec<CodeError>, text: &str, ctx: &Ctx) -> Json {
    let lines: Vec<usize> = text
        .split('\n')
        .map(|l| l.chars().count())
        .collect();
    let errors: Vec<Json> = errors
        .into_iter()
        .take(MAX_CODE_ERRORS)
        .map(|e| {
            let line = (e.line.max(1) as usize).min(lines.len());
            let len = lines[line - 1];
            let column = (e.column.max(1) as usize).min(len + 1);
            let length = (e.length as usize).min(len + 1 - column);
            json!({
                "line": line,
                "column": column,
                "length": length,
                "message": cut(ctx.t(&e.message), MAX_CODE_MESSAGE_CHARS),
            })
        })
        .collect();
    Json::Array(errors)
}

/// The completion action of an editor, behind the caps.
pub(crate) struct CompleteSource<A>(PhantomData<fn() -> A>);

impl<A> CompleteSource<A> {
    pub(crate) fn boxed() -> Box<dyn DataSource>
    where
        A: Action<Input = CodeQuery, Output = Result<Vec<Completion>>>,
    {
        Box::new(CompleteSource::<A>(PhantomData))
    }
}

impl<A> DataSource for CompleteSource<A>
where
    A: Action<Input = CodeQuery, Output = Result<Vec<Completion>>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let q = read_query(input)?;
            let items = A::call(ctx, q).await??;
            Ok(completions_json(items))
        })
    }
}

/// The check action of an editor, behind the caps.
pub(crate) struct CheckSource<A>(PhantomData<fn() -> A>);

impl<A> CheckSource<A> {
    pub(crate) fn boxed() -> Box<dyn DataSource>
    where
        A: Action<Input = CodeQuery, Output = Result<Vec<CodeError>>>,
    {
        Box::new(CheckSource::<A>(PhantomData))
    }
}

impl<A> DataSource for CheckSource<A>
where
    A: Action<Input = CodeQuery, Output = Result<Vec<CodeError>>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let q = read_query(input)?;
            let text = q.text.clone();
            let errors = A::call(ctx.clone(), q).await??;
            Ok(errors_json(errors, &text, &ctx))
        })
    }
}

/// What [`render_editor`] needs: the ids and attributes of the text area and the keys of the
/// completion and check sources of the node, when the user may run them.
pub(crate) struct EditorHtml<'a> {
    pub id: &'a str,
    /// Id, name, test id, `aria-describedby` and validation attributes of the text area.
    pub attrs: &'a str,
    pub lang: CodeLang,
    pub value: &'a str,
    pub rows: u32,
    pub complete: Option<&'a str>,
    pub check: Option<&'a str>,
}

/// Writes the editor: the text area (what the form submits and what works without a script),
/// the gutter and mirror the script fills, the suggestion list and the problem list.
pub(crate) fn render_editor(out: &mut String, r: &Render, e: &EditorHtml) {
    let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.code.editor.{key}")));
    let id = e.id;
    let kinds: serde_json::Map<String, Json> =
        ["keyword", "function", "table", "field", "value", "snippet"]
            .iter()
            .map(|k| (k.to_string(), Json::from(t(&format!("kind.{k}")))))
            .collect();
    let texts = json!({
        "count": t("count"),
        "none": t("none"),
        "problem": t("problem"),
        "kinds": kinds,
    });
    let attrs = if e
        .attrs
        .contains(r#"aria-describedby=""#)
    {
        e.attrs.replacen(
            r#"aria-describedby=""#,
            &format!(r#"aria-describedby="{id}-problems "#),
            1,
        )
    } else {
        format!(r#"{} aria-describedby="{id}-problems""#, e.attrs)
    };
    let complete = e
        .complete
        .map(|k| {
            format!(
                r#" data-complete="{}" aria-autocomplete="list" aria-controls="{id}-list""#,
                escape(k)
            )
        })
        .unwrap_or_default();
    let check = e
        .check
        .map(|k| format!(r#" data-check="{}""#, escape(k)))
        .unwrap_or_default();
    out.reserve(e.value.len() + 1024);
    out.push_str(&format!(
        r#"<div class="z-codeedit" data-zeyn-code="{}" data-max="{MAX_CODE_QUERY_BYTES}" data-texts="{}"><div class="z-codeedit-frame" dir="ltr"><pre class="z-codeedit-gutter" aria-hidden="true"></pre><div class="z-codeedit-area"><pre class="z-codeedit-mirror" aria-hidden="true"></pre><textarea class="z-codeedit-input" rows="{}" wrap="off" spellcheck="false" autocapitalize="off" autocomplete="off" autocorrect="off" translate="no"{complete}{check} {attrs}>&#10;{}</textarea></div>"#,
        e.lang.name(),
        attr(&texts),
        e.rows.clamp(2, 60),
        escape(e.value)
    ));
    if e.complete.is_some() {
        out.push_str(&format!(
            r#"<ul class="z-codeedit-list" id="{id}-list" role="listbox" aria-label="{}" hidden></ul>"#,
            t("suggestions")
        ));
    }
    out.push_str(&format!(
        r#"</div><div class="z-codeedit-problems" aria-live="polite"><ul id="{id}-problems"></ul></div><p class="z-sr-only" id="{id}-status" role="status"></p></div>"#
    ));
}

/// An editable code field outside a model form, such as the SQL of a report or the expression
/// of a calculated field: highlighting, line numbers, completion from an action
/// ([`CodeEditor::complete`]) and error markers from an action ([`CodeEditor::check`]). The text
/// area carries `name`, so a surrounding form or script reads it as a plain value. In a model
/// form use [`Control::Code`](super::Control::Code) with `Form::complete` and `Form::check`.
///
/// ```
/// use zeyn::ui::{CodeEditor, CodeLang};
/// use zeyn::{Component, I18n, Render};
///
/// let editor = CodeEditor::new("total", CodeLang::Expression)
///     .label("Total")
///     .value("[Orders.Total] * 1.18m");
/// let mut html = String::new();
/// editor.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains(r#"data-zeyn-code="expression""#));
/// assert!(html.contains(">&#10;[Orders.Total] * 1.18m</textarea>"));
/// ```
pub struct CodeEditor {
    name: String,
    lang: CodeLang,
    value: String,
    label: Option<Text>,
    rows: u32,
    readonly: bool,
    complete: Option<Box<dyn DataSource>>,
    check: Option<Box<dyn DataSource>>,
}

impl CodeEditor {
    /// An empty editor for `lang` whose text area is named `name`.
    pub fn new(name: impl Into<String>, lang: CodeLang) -> Self {
        CodeEditor {
            name: name.into(),
            lang,
            value: String::new(),
            label: None,
            rows: 8,
            readonly: false,
            complete: None,
            check: None,
        }
    }

    /// The text the editor starts with.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// The visible label. Without one the language names the field for screen readers.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Visible lines, from 2 to 60 (8 by default); the user can make it taller.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows;
        self
    }

    /// Shows the text without letting the user change it.
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// Suggestions while typing: after a pause, and at once with Ctrl+Space, the browser sends
    /// the text and the cursor to `action`, which returns [`Completion`]s. The list follows the
    /// APG combobox pattern: arrows move, Enter or Tab accept, Escape closes.
    ///
    /// ```ignore
    /// CodeEditor::new("total", CodeLang::Expression).complete(complete_fields)
    /// ```
    pub fn complete<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = CodeQuery, Output = Result<Vec<Completion>>>,
    {
        self.complete = Some(CompleteSource::<A>::boxed());
        self
    }

    /// Error markers: after a pause in typing the browser sends the text to `action`, which
    /// returns [`CodeError`]s; each is underlined and listed under the editor, where screen
    /// readers hear it.
    pub fn check<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = CodeQuery, Output = Result<Vec<CodeError>>>,
    {
        self.check = Some(CheckSource::<A>::boxed());
        self
    }
}

impl Component for CodeEditor {
    fn render(&self, out: &mut String, r: &Render) {
        let id = super::dom_id(r, "codeedit");
        let allowed = |s: &Option<Box<dyn DataSource>>| {
            s.as_ref()
                .is_some_and(|s| r.can(&s.action().access))
        };
        out.push_str(r#"<div class="z-field">"#);
        let name = match &self.label {
            Some(label) => {
                out.push_str(&format!(
                    r#"<label for="{id}" id="{id}-label">{}</label>"#,
                    r.text(label)
                ));
                String::new()
            }
            None => format!(
                r#" aria-label="{}""#,
                r.text(&Text::key(format!("zeyn.ui.code.{}", self.lang.name())))
            ),
        };
        let attrs = format!(
            r#"id="{id}" name="{}" data-testid="{}.code.{}"{name}{}"#,
            escape(&self.name),
            r.scope,
            escape(&self.name),
            if self.readonly { " readonly" } else { "" }
        );
        render_editor(
            out,
            r,
            &EditorHtml {
                id: &id,
                attrs: &attrs,
                lang: self.lang,
                value: &self.value,
                rows: self.rows,
                complete: allowed(&self.complete).then_some("complete"),
                check: allowed(&self.check).then_some("check"),
            },
        );
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "code_editor",
            "name": self.name,
            "lang": self.lang,
            "label": self.label,
            "complete": self.complete.as_ref().map(|s| s.action().name),
            "check": self.check.as_ref().map(|s| s.action().name),
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        match key {
            "complete" => self.complete.as_deref(),
            "check" => self.check.as_deref(),
            _ => None,
        }
    }
}
