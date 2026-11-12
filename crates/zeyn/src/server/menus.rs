//! Side menu entries built per request by a [`MenuSource`], for example from a database table,
//! merged after the static menus of the features. The source is never trusted: every request
//! checks access to each linked screen, and the source's answer is cut to the limits below before
//! it is cached.

use super::{AppState, NavGroup, catch_panic, locale};
use crate::ctx::{BoxFuture, Ctx, Service};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::Icon;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How many nodes of a source's answer are kept; the rest is dropped.
pub const MAX_MENU_NODES: usize = 500;
/// The longest label of a menu node, in characters. Longer plain labels are cut, longer catalog
/// keys drop their node.
pub const MAX_MENU_LABEL: usize = 100;
/// The longest path of a [`MenuNode::path`] link, in bytes.
pub const MAX_MENU_PATH: usize = 2048;
/// How long a source's answer is reused before the source is asked again.
pub const MENU_TTL: Duration = Duration::from_secs(60);
/// How long a source may take. A slower or failing source leaves the page with the static menu.
pub const MENU_TIMEOUT: Duration = Duration::from_secs(2);
/// How many answers the menu cache keeps; the oldest one goes first.
pub const MAX_MENU_CACHE: usize = 256;

/// Builds side menu entries for a request, from a database table, an API or the user's roles.
/// Register one with `App::menu_source`; an async closure that takes a [`Ctx`] is a source.
///
/// ```
/// use zeyn::prelude::*;
///
/// let app = App::new("Admin").menu_source(|ctx: Ctx| async move {
///     let mut links = vec![MenuNode::screen("Orders", "orders")];
///     if ctx.identity().permissions.contains("reports.read") {
///         links.push(MenuNode::path("Sales report", "/reports/sales"));
///     }
///     Ok(vec![MenuNode::group("Shortcuts", links).icon(Icon::Link)])
/// });
/// ```
pub trait MenuSource: Send + Sync + 'static {
    /// The menu for the request's user, tenant and language: groups at the top level, with links
    /// and nested groups in them.
    fn menu(&self, ctx: &Ctx) -> BoxFuture<crate::Result<Vec<MenuNode>>>;
}

impl<F, Fut> MenuSource for F
where
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = crate::Result<Vec<MenuNode>>> + Send + 'static,
{
    fn menu(&self, ctx: &Ctx) -> BoxFuture<crate::Result<Vec<MenuNode>>> {
        Box::pin(self(ctx.clone()))
    }
}

/// One entry of a [`MenuSource`]'s menu: a group or a link. Labels are catalog keys
/// ([`Text::key`]) or plain text, which is escaped like any other text.
#[derive(Clone, Debug)]
pub struct MenuNode {
    label: Text,
    icon: Option<Icon>,
    permission: Option<String>,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Group { open: bool, children: Vec<MenuNode> },
    Screen { name: String, params: Json },
    Path(String),
}

impl MenuNode {
    /// A group with these links and nested groups. Top level nodes must be groups; groups nest
    /// [`crate::MENU_DEPTH`] levels deep at most. Links come before nested groups, as in static
    /// menus, and a group whose links the user may not open is left out.
    pub fn group(label: impl Into<Text>, children: Vec<MenuNode>) -> Self {
        Self::new(
            label,
            Kind::Group {
                open: false,
                children,
            },
        )
    }

    /// A link to the registered screen of this name. It is left out when the app has no such
    /// screen, when the user may not open it, or when its route has path segments that
    /// [`Self::params`] does not fill.
    pub fn screen(label: impl Into<Text>, name: impl Into<String>) -> Self {
        Self::new(
            label,
            Kind::Screen {
                name: name.into(),
                params: Json::Null,
            },
        )
    }

    /// A link to a path of the app, such as `/reports/sales?year=2026`, below the base path. Only
    /// relative paths that start with one `/` are kept: no scheme, no `//`, no backslash and no
    /// control characters.
    pub fn path(label: impl Into<Text>, path: impl Into<String>) -> Self {
        Self::new(label, Kind::Path(path.into()))
    }

    fn new(label: impl Into<Text>, kind: Kind) -> Self {
        MenuNode {
            label: label.into(),
            icon: None,
            permission: None,
            kind,
        }
    }

    /// The params of a screen link: path segments and the `params` query, as in
    /// [`crate::screen_url`].
    pub fn params(mut self, value: Json) -> Self {
        if let Kind::Screen { params, .. } = &mut self.kind {
            *params = value;
        }
        self
    }

    /// The icon before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Keeps a group open, without a toggle button.
    pub fn open(mut self) -> Self {
        if let Kind::Group { open, .. } = &mut self.kind {
            *open = true;
        }
        self
    }

    /// Shows the node only to users with this permission, on top of the screen's own access.
    pub fn permission(mut self, permission: impl Into<String>) -> Self {
        self.permission = Some(permission.into());
        self
    }
}

/// The menu source of the app and the cache of its answers. Inject it into an action and call
/// [`Menus::invalidate`] after the menu's data changed.
#[derive(Clone)]
pub struct Menus(Arc<Inner>);

struct Inner {
    source: Box<dyn MenuSource>,
    per_user: bool,
    generation: AtomicU64,
    cache: Mutex<HashMap<String, CachedMenu>>,
}

/// A source's answer and when it was loaded.
type CachedMenu = (Instant, Arc<Vec<MenuNode>>);

impl Service for Menus {}

impl Menus {
    pub(super) fn new(source: Box<dyn MenuSource>, per_user: bool) -> Self {
        Menus(Arc::new(Inner {
            source,
            per_user,
            generation: AtomicU64::new(0),
            cache: Mutex::new(HashMap::new()),
        }))
    }

    /// Forgets every cached answer, so the next page of each user asks the source again.
    pub fn invalidate(&self) {
        self.0
            .generation
            .fetch_add(1, Ordering::SeqCst);
        if let Ok(mut c) = self.0.cache.lock() {
            c.clear();
        }
    }

    /// The cache key: tenant, language, permissions and, for per user sources, the user.
    fn key(&self, ctx: &Ctx) -> String {
        let id = ctx.identity();
        let mut key = format!(
            "{}\u{1f}{}\u{1f}{}{}",
            ctx.tenant()
                .map(|t| t.id())
                .unwrap_or_default(),
            ctx.locale(),
            u8::from(id.superuser),
            u8::from(id.user.is_some()),
        );
        for p in &id.permissions {
            key.push('\u{1e}');
            key.push_str(p);
        }
        if self.0.per_user {
            key.push('\u{1f}');
            key.push_str(id.user.as_deref().unwrap_or_default());
        }
        key
    }

    /// The source's answer for this request, from the cache when it is fresh. Empty when the
    /// source fails, panics or takes longer than [`MENU_TIMEOUT`]; failures are not cached.
    async fn load(&self, st: &AppState, ctx: &Ctx) -> Arc<Vec<MenuNode>> {
        let key = self.key(ctx);
        let cached = self
            .0
            .cache
            .lock()
            .ok()
            .and_then(|c| c.get(&key).cloned());
        if let Some((at, nodes)) = cached
            && at.elapsed() < MENU_TTL
        {
            return nodes;
        }
        let generation = self.0.generation.load(Ordering::SeqCst);
        let answer = tokio::time::timeout(MENU_TIMEOUT, catch_panic(self.0.source.menu(ctx))).await;
        let nodes = match answer {
            Ok(Ok(Ok(nodes))) => {
                let mut budget = MAX_MENU_NODES;
                Arc::new(clean(st, nodes, 0, &mut budget))
            }
            Ok(Ok(Err(e))) => {
                tracing::warn!("menu source failed, showing the static menu: {e}");
                return Arc::default();
            }
            Ok(Err(_)) => {
                tracing::error!("menu source panicked, showing the static menu");
                return Arc::default();
            }
            Err(_) => {
                tracing::warn!(
                    "menu source took longer than {MENU_TIMEOUT:?}, showing the static menu"
                );
                return Arc::default();
            }
        };
        if let Ok(mut c) = self.0.cache.lock()
            && self.0.generation.load(Ordering::SeqCst) == generation
        {
            if c.len() >= MAX_MENU_CACHE && !c.contains_key(&key) {
                c.retain(|_, (at, _)| at.elapsed() < MENU_TTL);
                if c.len() >= MAX_MENU_CACHE
                    && let Some(oldest) = c
                        .iter()
                        .min_by_key(|(_, (at, _))| *at)
                        .map(|(k, _)| k.clone())
                {
                    c.remove(&oldest);
                }
            }
            c.insert(key, (Instant::now(), nodes.clone()));
        }
        nodes
    }
}

/// The nodes that pass the structural limits: groups at the top level and at most
/// [`crate::MENU_DEPTH`] levels below it, links only inside groups, known screens, safe paths,
/// short labels and [`MAX_MENU_NODES`] nodes in all.
fn clean(st: &AppState, nodes: Vec<MenuNode>, depth: usize, budget: &mut usize) -> Vec<MenuNode> {
    let mut out = Vec::new();
    for mut node in nodes {
        if *budget == 0 {
            break;
        }
        if !short_label(&mut node.label) {
            continue;
        }
        match &mut node.kind {
            Kind::Group { children, .. } => {
                if depth > crate::MENU_DEPTH {
                    continue;
                }
                *budget -= 1;
                *children = clean(st, std::mem::take(children), depth + 1, budget);
                if children.is_empty() {
                    continue;
                }
            }
            Kind::Screen { name, .. } => {
                if depth == 0 {
                    continue;
                }
                if !st.screens.contains_key(name.as_str()) {
                    tracing::warn!("menu source links to unknown screen `{name}`, left out");
                    continue;
                }
                *budget -= 1;
            }
            Kind::Path(path) => {
                if depth == 0 || !safe_path(path) {
                    continue;
                }
                *budget -= 1;
            }
        }
        out.push(node);
    }
    out
}

/// Whether a label fits [`MAX_MENU_LABEL`], cutting plain text that is longer.
fn short_label(label: &mut Text) -> bool {
    match label {
        Text::Literal(s) => {
            if let Some((at, _)) = s.char_indices().nth(MAX_MENU_LABEL) {
                s.to_mut().truncate(at);
            }
            !s.trim().is_empty()
        }
        Text::Key { key, .. } => !key.is_empty() && key.len() <= MAX_MENU_LABEL,
    }
}

/// Whether `path` is a relative path on the app's origin.
pub(crate) fn safe_path(path: &str) -> bool {
    path.len() <= MAX_MENU_PATH
        && path.starts_with('/')
        && !path.starts_with("//")
        && !path
            .chars()
            .any(|c| c == '\\' || c.is_control())
}

impl AppState {
    /// The dynamic menu of a page showing `current`: the answer of the app's [`MenuSource`].
    /// Empty without a source and for screens in the site shell, which has no side menu.
    pub(super) async fn dynamic_menu(
        &self,
        ctx: &Ctx,
        current: Option<&str>,
    ) -> Arc<Vec<MenuNode>> {
        let site = current.is_some_and(|name| {
            self.screens
                .get(name)
                .is_some_and(|(fi, _)| {
                    self.features[*fi]
                        .shell_of(name)
                        .is_some()
                })
        });
        match self.services.get::<Menus>() {
            Some(menus) if !site => menus.load(self, ctx).await,
            _ => Arc::default(),
        }
    }

    /// The groups of the dynamic menu that the user sees: links to screens the user may not open
    /// (access, feature guards, the node's permission) are left out, and so are groups left
    /// without links.
    pub(super) fn dynamic_groups(
        &self,
        nodes: &[MenuNode],
        ctx: &Ctx,
        r: &Render,
        current: Option<&str>,
    ) -> Vec<NavGroup> {
        nodes
            .iter()
            .filter_map(|n| self.dynamic_group(n, ctx, r, current))
            .collect()
    }

    fn dynamic_group(
        &self,
        node: &MenuNode,
        ctx: &Ctx,
        r: &Render,
        current: Option<&str>,
    ) -> Option<NavGroup> {
        let Kind::Group { open, children } = &node.kind else {
            return None;
        };
        if !permitted(ctx, node) {
            return None;
        }
        let mut group = NavGroup::new(r.text(&node.label), *open);
        let mut icons = false;
        for child in children {
            if !permitted(ctx, child) {
                continue;
            }
            let here = match &child.kind {
                Kind::Group { .. } => {
                    group
                        .groups
                        .extend(self.dynamic_group(child, ctx, r, current));
                    continue;
                }
                Kind::Screen { name, params } => {
                    let Some(href) = self.screen_href(ctx, name, params) else {
                        continue;
                    };
                    let here = current == Some(name.as_str());
                    group.links.push(super::nav_link(
                        &href,
                        here,
                        child.icon,
                        &r.text(&child.label),
                        "",
                    ));
                    here
                }
                Kind::Path(path) => {
                    let href = escape(&self.url(&locale::path(self, ctx, path)));
                    group.links.push(super::nav_link(
                        &href,
                        false,
                        child.icon,
                        &r.text(&child.label),
                        "",
                    ));
                    false
                }
            };
            icons |= child.icon.is_some();
            if here {
                group.here = true;
                group.page = Some(r.text(&child.label));
            }
        }
        if group.links.is_empty() && group.groups.is_empty() {
            return None;
        }
        group.here |= group.groups.iter().any(|g| g.here);
        if !icons && let Some(icon) = node.icon {
            group.icon = icon.html();
        }
        Some(group)
    }

    /// The escaped address of a screen link the user may open, or `None`.
    fn screen_href(&self, ctx: &Ctx, name: &str, params: &Json) -> Option<String> {
        let (fi, entry) = self.screens.get(name)?;
        let feature = &self.features[*fi];
        if !ctx.identity().can(&entry.meta.access) || feature.check_guards(ctx).is_err() {
            return None;
        }
        let url = crate::screen::screen_url(&feature.path_of(&entry.meta)?, params)?;
        Some(escape(&self.url(&locale::path(self, ctx, &url))))
    }
}

/// Whether the user holds the node's own permission, if it has one.
fn permitted(ctx: &Ctx, node: &MenuNode) -> bool {
    let id = ctx.identity();
    node.permission
        .as_ref()
        .is_none_or(|p| id.superuser || id.permissions.contains(p))
}
