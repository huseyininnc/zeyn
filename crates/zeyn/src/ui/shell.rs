//! Desktop class parts: `MenuBar`, `Toolbox`, `PropertyInspector` and `StatusBar`. They render
//! on the server like every component and work inside the layout containers.

use super::dom_id;
use crate::command::Command;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, FormSource, Icon, Node};
use serde_json::{Value as Json, json};

super::layout::into_node!(MenuBar, Toolbox, PropertyInspector, StatusBar);

#[allow(
    clippy::large_enum_variant,
    reason = "nearly every entry of the list is the large variant, so boxing it would add an allocation per entry and save no memory"
)]
enum Entry {
    Command(Command),
    Separator,
    Panels,
}

/// Items of one menu of a [`MenuBar`].
#[derive(Default)]
pub struct MenuList {
    entries: Vec<Entry>,
}

impl MenuList {
    /// A command. Users who may not run it do not see it. Register action commands on the
    /// feature too (`Feature::command`): the server runs them by id.
    pub fn command(mut self, command: Command) -> Self {
        self.entries
            .push(Entry::Command(command));
        self
    }

    /// A line between groups of items.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// The panels of the [`super::Dock`] on the same screen as check items: unchecking one
    /// closes the panel, checking it brings the panel back where it was.
    pub fn panels(mut self) -> Self {
        self.entries.push(Entry::Panels);
        self
    }
}

/// A menu bar built from commands (W3C APG menubar pattern).
///
/// Keyboard: Left and Right move between menus, Down, Enter or Space open one, Up opens it at the
/// last item, Home and End jump, a letter moves to the next item that starts with it, Escape
/// closes and returns to the menu bar, Tab leaves it. Shortcuts of the commands are shown and
/// work while the menus are closed.
///
/// ```ignore
/// MenuBar::new("Main").menu("File", |m| m.command(new_order()).separator().command(archive()))
///     .menu("View", |m| m.panels())
/// ```
pub struct MenuBar {
    label: Text,
    menus: Vec<(Text, MenuList)>,
}

impl MenuBar {
    /// A menu bar with an accessible name, such as "Main".
    pub fn new(label: impl Into<Text>) -> Self {
        Self {
            label: label.into(),
            menus: Vec::new(),
        }
    }

    pub fn menu(
        mut self,
        title: impl Into<Text>,
        build: impl FnOnce(MenuList) -> MenuList,
    ) -> Self {
        self.menus
            .push((title.into(), build(MenuList::default())));
        self
    }
}

fn icon_html(icon: Option<Icon>) -> String {
    icon.map(Icon::html).unwrap_or_default()
}

/// Items of one menu as the user may see them, without doubled or dangling separators.
fn menu_items(list: &MenuList, r: &Render) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut last_sep = true;
    for e in &list.entries {
        match e {
            Entry::Separator => {
                if !last_sep {
                    out.push(r#"<li role="separator" class="z-menu-sep"></li>"#.to_string());
                    last_sep = true;
                }
            }
            Entry::Panels => {
                out.push(r#"<li role="none" class="z-menubar-panels" data-zeyn-dock-panels hidden></li>"#.to_string());
                last_sep = false;
            }
            Entry::Command(c) => {
                if !r.can(&c.access()) {
                    continue;
                }
                let hint = c
                    .hint()
                    .map(|h| {
                        format!(
                            r#"<span class="z-menu-hint" aria-hidden="true">{}</span>"#,
                            escape(&h)
                        )
                    })
                    .unwrap_or_default();
                out.push(format!(
                    r#"<li role="none"><button type="button" role="menuitem" class="z-menu-item" tabindex="-1" data-testid="{}.menubar.{}"{}>{}<span class="z-menu-label">{}</span>{hint}</button></li>"#,
                    r.scope,
                    escape(c.id()),
                    c.attrs(r),
                    icon_html(c.icon_of()),
                    r.text(c.label())
                ));
                last_sep = false;
            }
        }
    }
    if last_sep {
        out.pop_if(|s| s.contains(r#"role="separator""#));
    }
    out
}

impl Component for MenuBar {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "menubar");
        let menus: Vec<(String, Vec<String>)> = self
            .menus
            .iter()
            .map(|(t, l)| (r.text(t), menu_items(l, r)))
            .filter(|(_, items)| !items.is_empty())
            .collect();
        out.push_str(&format!(
            r#"<ul class="z-menubar" role="menubar" aria-label="{}" data-zeyn-menubar data-testid="{}.menubar">"#,
            r.text(&self.label),
            r.scope
        ));
        for (i, (title, items)) in menus.iter().enumerate() {
            out.push_str(&format!(
                r#"<li role="none" class="z-menubar-top"><button type="button" role="menuitem" class="z-menubar-item" id="{id}-{i}" aria-haspopup="menu" aria-expanded="false" aria-controls="{id}-{i}-menu" tabindex="{}">{title}</button><ul role="menu" class="z-menu z-menubar-menu" id="{id}-{i}-menu" aria-labelledby="{id}-{i}" hidden>{}</ul></li>"#,
                if i == 0 { 0 } else { -1 },
                items.join("")
            ));
        }
        out.push_str("</ul>");
    }

    fn schema(&self) -> Json {
        let menus: Vec<Json> = self
            .menus
            .iter()
            .map(|(t, l)| {
                let items: Vec<Json> = l
                    .entries
                    .iter()
                    .map(|e| match e {
                        Entry::Command(c) => json!({ "command": c.id() }),
                        Entry::Separator => json!({ "separator": true }),
                        Entry::Panels => json!({ "panels": true }),
                    })
                    .collect();
                json!({ "title": t, "items": items })
            })
            .collect();
        json!({ "type": "menubar", "label": self.label, "menus": menus })
    }
}

/// A vertical group of tool buttons of which one is on, as in a drawing program. Choosing a tool
/// sends the `zeyn:tool` event (`detail.tool` is its id) and tells the screen's status bar.
///
/// Keyboard: Up and Down (or Left and Right) move between tools, Home and End jump, Enter or
/// Space choose.
pub struct Toolbox {
    label: Text,
    tools: Vec<(&'static str, Text, Option<Icon>)>,
    selected: Option<&'static str>,
}

impl Toolbox {
    pub fn new(label: impl Into<Text>) -> Self {
        Self {
            label: label.into(),
            tools: Vec::new(),
            selected: None,
        }
    }

    /// A tool. Without an icon the button shows the first letter of its label.
    pub fn tool(mut self, id: &'static str, label: impl Into<Text>, icon: Option<Icon>) -> Self {
        self.tools
            .push((id, label.into(), icon));
        self
    }

    /// The tool that is on at first. The first tool otherwise.
    pub fn selected(mut self, id: &'static str) -> Self {
        self.selected = Some(id);
        self
    }
}

impl Component for Toolbox {
    fn render(&self, out: &mut String, r: &Render) {
        let on = self
            .selected
            .or(self.tools.first().map(|t| t.0));
        out.push_str(&format!(
            r#"<div class="z-toolbox" role="toolbar" aria-orientation="vertical" aria-label="{}" data-zeyn-toolbox data-text-selected="{}" data-testid="{}.toolbox">"#,
            r.text(&self.label),
            r.text(&Text::key("zeyn.ui.toolbox.selected")),
            r.scope
        ));
        for (id, label, icon) in &self.tools {
            let label = r.text(label);
            let pressed = on == Some(*id);
            let face = match icon {
                Some(i) => i.html(),
                None => format!(
                    r#"<span aria-hidden="true">{}</span>"#,
                    escape(
                        &label
                            .chars()
                            .next()
                            .map(String::from)
                            .unwrap_or_default()
                    )
                ),
            };
            out.push_str(&format!(
                r#"<button type="button" class="z-tool" aria-pressed="{pressed}" tabindex="{}" data-tool="{}" aria-label="{label}" title="{label}" data-testid="{}.tool.{}">{face}</button>"#,
                if pressed { 0 } else { -1 },
                escape(id),
                r.scope,
                escape(id)
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        let tools: Vec<Json> = self
            .tools
            .iter()
            .map(|(id, label, icon)| json!({ "id": id, "label": label, "icon": icon }))
            .collect();
        json!({ "type": "toolbox", "label": self.label, "tools": tools, "selected": self.selected })
    }
}

/// Edits the fields of a model in a narrow column, grouped in foldable sections, as a property
/// panel does. It wraps a form: build the form with [`crate::Form::section`] and `submit`, so
/// saving goes through the form's own validation and `POST /_zeyn/form`.
///
/// ```ignore
/// PropertyInspector::new("Shape", Form::of(shape)
///     .section("Position").row(|r| r.field(Shape::x).field(Shape::y))
///     .section("Look").field(Shape::fill)
///     .submit("Apply", save_shape))
/// ```
pub struct PropertyInspector {
    title: Text,
    form: Node,
}

impl PropertyInspector {
    pub fn new(title: impl Into<Text>, form: impl Into<Node>) -> Self {
        Self {
            title: title.into(),
            form: form.into(),
        }
    }
}

impl Component for PropertyInspector {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "inspector");
        out.push_str(&format!(
            r#"<section class="z-inspector" aria-labelledby="{id}-title"><h2 class="z-inspector-title" id="{id}-title">{}</h2>"#,
            r.text(&self.title)
        ));
        self.form.render(out, r);
        out.push_str("</section>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "property_inspector", "title": self.title, "form": self.form.schema() })
    }

    fn form(&self) -> Option<&dyn FormSource> {
        self.form.form()
    }

    fn source(&self, key: &str) -> Option<&dyn super::DataSource> {
        self.form.source(key)
    }

    fn reveal(&self, field: &str) -> Option<crate::privacy::Revealed> {
        self.form.reveal(field)
    }
}

/// A bar at the bottom of a work area with items at the start and the end and one live text
/// that screen readers announce when it changes. Scripts change it with the `zeyn:status` event
/// (`detail.text`) sent from any element of the screen.
pub struct StatusBar {
    label: Text,
    start: Vec<Text>,
    end: Vec<Text>,
    live: Option<Text>,
}

impl StatusBar {
    pub fn new(label: impl Into<Text>) -> Self {
        Self {
            label: label.into(),
            start: Vec::new(),
            end: Vec::new(),
            live: None,
        }
    }

    /// An item at the start (the left in left to right languages).
    pub fn left(mut self, text: impl Into<Text>) -> Self {
        self.start.push(text.into());
        self
    }

    /// An item at the end.
    pub fn right(mut self, text: impl Into<Text>) -> Self {
        self.end.push(text.into());
        self
    }

    /// The first live text.
    pub fn live(mut self, text: impl Into<Text>) -> Self {
        self.live = Some(text.into());
        self
    }
}

impl Component for StatusBar {
    fn render(&self, out: &mut String, r: &Render) {
        let items = |list: &[Text]| {
            list.iter()
                .map(|t| format!(r#"<span class="z-statusbar-item">{}</span>"#, r.text(t)))
                .collect::<String>()
        };
        out.push_str(&format!(
            r#"<div class="z-statusbar" role="group" aria-label="{}" data-zeyn-statusbar data-testid="{}.statusbar"><div class="z-statusbar-start">{}<span class="z-statusbar-item z-statusbar-live" role="status" aria-live="polite" data-zeyn-status-live>{}</span></div><div class="z-statusbar-end">{}</div></div>"#,
            r.text(&self.label),
            r.scope,
            items(&self.start),
            self.live.as_ref().map(|t| r.text(t)).unwrap_or_default(),
            items(&self.end)
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "statusbar", "label": self.label, "start": self.start, "end": self.end, "live": self.live })
    }
}
