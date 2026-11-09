//! [`DiffView`]: the fields that changed between two values of a record, side by side or inline.

use super::layout::into_node;
use crate::audit::FieldChange;
use crate::i18n::Text;
use crate::model::{FieldMeta, Model};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use crate::value::{FieldKind, Value};
use serde_json::{Value as Json, json};

into_node!(DiffView);

/// Most changed fields one [`DiffView`] shows; more are left out with a note.
pub const MAX_DIFF_FIELDS: usize = 200;

/// How a [`DiffView`] lays out the old and the new value of a field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiffLayout {
    /// A table with a column for the old values and one for the new values.
    #[default]
    SideBySide,
    /// One entry per field with the old value struck out and the new value after it, for narrow
    /// places such as a timeline or a window.
    Inline,
}

/// The fields that changed between two values of a record: the old value struck out, the new
/// one marked as added. Built from an audit record ([`DiffView::changes`], the
/// [`FieldChange`]s an `AuditEntry` carries) or from two values of a model ([`DiffView::of`],
/// which compares them with [`crate::audit::diff`]).
///
/// Masked fields stay masked: values are the masked text the audit trail holds, and a field
/// whose model marks it `#[mask]` is masked again before it is shown, so no raw value reaches the
/// page, even from a hand built change list. Rendered on the server; it needs no script.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::DiffView;
/// use zeyn::{Component, I18n, Render};
///
/// #[model]
/// pub struct Customer {
///     pub name: String,
///     pub city: String,
/// }
///
/// let old = Customer { name: "Ada".into(), city: "London".into() };
/// let new = Customer { name: "Ada".into(), city: "Paris".into() };
/// let mut html = String::new();
/// DiffView::of(&old, &new).render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains("<del>London</del>") && html.contains("<ins>Paris</ins>"));
/// assert!(!html.contains("Ada"), "unchanged fields are left out");
/// ```
pub struct DiffView {
    changes: Vec<FieldChange>,
    fields: &'static [FieldMeta],
    title: Option<Text>,
    layout: DiffLayout,
    dropped: usize,
}

impl DiffView {
    /// The fields that differ between `old` and `new`, in declaration order. Row versions are
    /// left out and masked fields are masked, as in the audit trail.
    pub fn of<M: Model>(old: &M, new: &M) -> Self {
        DiffView::changes(crate::audit::diff(old, new)).model::<M>()
    }

    /// The changes of an audit record. Without [`DiffView::model`] the field names are the
    /// labels.
    pub fn changes(changes: impl IntoIterator<Item = FieldChange>) -> Self {
        let mut changes: Vec<FieldChange> = changes.into_iter().collect();
        let dropped = changes
            .len()
            .saturating_sub(MAX_DIFF_FIELDS);
        changes.truncate(MAX_DIFF_FIELDS);
        DiffView {
            changes,
            fields: &[],
            title: None,
            layout: DiffLayout::SideBySide,
            dropped,
        }
    }

    /// The model the changes belong to: fields get their labels in the user's language, choices
    /// their option labels, and fields marked `#[mask]` are masked again.
    pub fn model<M: Model>(mut self) -> Self {
        self.fields = M::fields();
        self
    }

    /// A heading over the changes, which also names the table for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Side by side (the default) or inline.
    pub fn layout(mut self, layout: DiffLayout) -> Self {
        self.layout = layout;
        self
    }

    fn meta(&self, field: &str) -> Option<&'static FieldMeta> {
        self.fields
            .iter()
            .find(|m| m.name == field)
    }

    /// The label of a field, escaped.
    fn label(&self, r: &Render, field: &str) -> String {
        match self.meta(field) {
            Some(m) => r.label(m),
            None => escape(field),
        }
    }

    /// A value as shown, escaped, or the empty marker. Masked fields of the model are masked
    /// again; choices show their option label.
    fn shown(&self, r: &Render, field: &str, value: Option<&str>) -> String {
        let Some(text) = value else {
            return format!(
                r#"<span class="z-diff-empty">{}</span>"#,
                r.text(&Text::key("zeyn.ui.diff.empty"))
            );
        };
        let meta = self.meta(field);
        if let Some(m) = meta.filter(|m| m.mask.is_some()) {
            return escape(&crate::privacy::masked_text(m, text));
        }
        if let Some(m) = meta
            && let FieldKind::Choice(options) = &m.kind
            && let Some(o) = options.iter().find(|o| o.value == text)
        {
            return r.value(&Value::Choice(o.value), &m.kind);
        }
        escape(text)
    }
}

impl Component for DiffView {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.diff.{key}")));
        let id = super::dom_id(r, "diff");
        out.push_str(r#"<section class="z-diff">"#);
        if let Some(title) = &self.title {
            out.push_str(&format!(
                r#"<h2 class="z-diff-title" id="{id}-title">{}</h2>"#,
                r.text(title)
            ));
        }
        if self.changes.is_empty() {
            out.push_str(&format!(
                r#"<p class="z-diff-none">{}</p></section>"#,
                t("none")
            ));
            return;
        }
        let labelled = match self.title {
            Some(_) => format!(r#" aria-labelledby="{id}-title""#),
            None => format!(r#" aria-label="{}""#, t("label")),
        };
        match self.layout {
            DiffLayout::SideBySide => {
                out.push_str(&format!(
                    r#"<div class="z-diff-scroll"><table class="z-diff-table"{labelled}><thead><tr><th scope="col">{}</th><th scope="col">{}</th><th scope="col">{}</th></tr></thead><tbody>"#,
                    t("field"),
                    t("before"),
                    t("after")
                ));
                for c in &self.changes {
                    out.push_str(&format!(
                        r#"<tr><th scope="row">{}</th><td class="z-diff-old"><del>{}</del></td><td class="z-diff-new"><ins>{}</ins></td></tr>"#,
                        self.label(r, c.field),
                        self.shown(r, c.field, c.old.as_deref()),
                        self.shown(r, c.field, c.new.as_deref())
                    ));
                }
                out.push_str("</tbody></table></div>");
            }
            DiffLayout::Inline => {
                out.push_str(&format!(r#"<dl class="z-diff-list"{labelled}>"#));
                for c in &self.changes {
                    out.push_str(&format!(
                        r#"<div class="z-diff-item"><dt>{}</dt><dd><span class="z-sr-only">{} </span><del>{}</del> <span class="z-diff-arrow" aria-hidden="true">&rarr;</span> <span class="z-sr-only">{} </span><ins>{}</ins></dd></div>"#,
                        self.label(r, c.field),
                        t("before"),
                        self.shown(r, c.field, c.old.as_deref()),
                        t("after"),
                        self.shown(r, c.field, c.new.as_deref())
                    ));
                }
                out.push_str("</dl>");
            }
        }
        if self.dropped > 0 {
            out.push_str(&format!(
                r#"<p class="z-diff-more">{}</p>"#,
                r.text(&Text::key("zeyn.ui.diff.more").arg("n", self.dropped.to_string()))
            ));
        }
        out.push_str("</section>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "diff_view",
            "title": self.title,
            "layout": match self.layout {
                DiffLayout::SideBySide => "side_by_side",
                DiffLayout::Inline => "inline",
            },
            "fields": self.changes.iter().map(|c| c.field).collect::<Vec<_>>(),
        })
    }
}
