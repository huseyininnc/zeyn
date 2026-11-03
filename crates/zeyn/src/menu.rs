//! Context menus: built on the server per row or per screen, shown by the runtime on right click,
//! Shift+F10 or the context menu key.

use crate::action::{Action, ActionMeta};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Effect, Icon, RowCall};
use serde_json::{Value as Json, json};
use std::sync::Mutex;

type Run = Mutex<Option<Box<dyn FnOnce(Ctx) -> BoxFuture<crate::action::Result<()>> + Send>>>;

enum Kind {
    Effect(Effect),
    Action { meta: ActionMeta, run: Run },
    Sub(ContextMenu),
    Separator,
}

struct Item {
    label: Text,
    icon: Option<Icon>,
    hint: Option<String>,
    disabled: bool,
    kind: Kind,
}

/// A context menu. Items run an effect in the browser or an action on the server; action items
/// carry the action's permission and are left out for users who may not run it.
///
/// ```ignore
/// .row_menu(|p: &Product| {
///     ContextMenu::new()
///         .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
///         .icon(Icon::Edit)
///         .separator()
///         .action(t!("stock.restock"), restock_product, p.id)
/// })
/// ```
#[derive(Default)]
pub struct ContextMenu {
    items: Vec<Item>,
}

impl ContextMenu {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(mut self, label: Text, kind: Kind) -> Self {
        self.items.push(Item {
            label,
            icon: None,
            hint: None,
            disabled: false,
            kind,
        });
        self
    }

    /// An item that runs an effect, such as opening a window.
    pub fn item(self, label: impl Into<Text>, effect: Effect) -> Self {
        self.push(label.into(), Kind::Effect(effect))
    }

    /// An item that runs `action` with `input` on the server. The input never travels to the
    /// browser: when the item is picked, the server builds the menu again (for a row, from the row
    /// loaded again by key) and runs this item's action after checking its permission.
    pub fn action<A>(self, label: impl Into<Text>, _action: A, input: A::Input) -> Self
    where
        A: Action,
        A::Output: Outcome,
    {
        let run: Box<dyn FnOnce(Ctx) -> BoxFuture<crate::action::Result<()>> + Send> =
            Box::new(move |ctx| {
                Box::pin(async move {
                    let output = A::call(ctx, input).await?;
                    output.failure().map_or(Ok(()), Err)
                })
            });
        self.push(
            label.into(),
            Kind::Action {
                meta: A::META,
                run: Mutex::new(Some(run)),
            },
        )
    }

    /// A submenu.
    pub fn submenu(self, label: impl Into<Text>, menu: ContextMenu) -> Self {
        self.push(label.into(), Kind::Sub(menu))
    }

    /// A line between groups of items.
    pub fn separator(self) -> Self {
        self.push(Text::from(""), Kind::Separator)
    }

    /// Icon of the last item.
    pub fn icon(mut self, icon: Icon) -> Self {
        if let Some(i) = self.items.last_mut() {
            i.icon = Some(icon);
        }
        self
    }

    /// Shortcut shown beside the last item, such as `"Ctrl+E"`. Only a hint; give the toolbar
    /// button the real shortcut.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        if let Some(i) = self.items.last_mut() {
            i.hint = Some(keys.into());
        }
        self
    }

    /// Shows the last item but does not let it run.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if let Some(i) = self.items.last_mut() {
            i.disabled = disabled;
        }
        self
    }

    /// Whether the menu has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Items for the runtime, in the user's language, without action items the user may not
    /// run, empty submenus and doubled or dangling separators. Action items carry their path
    /// (`"0"`, `"2.1"`) in the full menu, so leaving items out never shifts what runs.
    pub fn to_json(&self, r: &Render) -> Json {
        Json::Array(self.items_json(r, ""))
    }

    fn items_json(&self, r: &Render, prefix: &str) -> Vec<Json> {
        let mut out: Vec<Json> = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            let path = format!("{prefix}{i}");
            let mut entry = match &item.kind {
                Kind::Separator => {
                    if out
                        .last()
                        .is_some_and(|e| e.get("separator").is_none())
                    {
                        out.push(json!({ "separator": true }));
                    }
                    continue;
                }
                Kind::Effect(e) => json!({ "effect": e.to_json(r) }),
                Kind::Action { meta, .. } => {
                    if !r.can(&meta.access) {
                        continue;
                    }
                    json!({ "run": path })
                }
                Kind::Sub(menu) => {
                    let items = menu.items_json(r, &format!("{path}."));
                    if items.is_empty() {
                        continue;
                    }
                    json!({ "items": items })
                }
            };
            entry["label"] = json!(r.i18n.text(r.locale, &item.label));
            if let Some(icon) = item.icon {
                entry["icon"] = serde_json::to_value(icon).unwrap_or(Json::Null);
            }
            if let Some(h) = &item.hint {
                entry["hint"] = json!(h);
            }
            if item.disabled {
                entry["disabled"] = json!(true);
            }
            out.push(entry);
        }
        while out
            .last()
            .is_some_and(|e| e.get("separator").is_some())
        {
            out.pop();
        }
        out
    }

    /// Takes the action item at `path` (`"2.1"`) to run it. `None` for unknown paths, effect
    /// items, submenus and disabled items.
    pub fn take(&self, path: &str) -> Option<RowCall> {
        let mut menu = self;
        let mut parts = path.split('.').peekable();
        while let Some(part) = parts.next() {
            let item = menu
                .items
                .get(part.parse::<usize>().ok()?)?;
            match (&item.kind, parts.peek()) {
                (Kind::Sub(sub), Some(_)) => menu = sub,
                (Kind::Action { meta, run }, None) if !item.disabled => {
                    let run = run.lock().ok()?.take()?;
                    return Some(RowCall { meta: *meta, run });
                }
                _ => return None,
            }
        }
        None
    }
}
