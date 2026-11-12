use crate::action::{Action, Error, Result};
use crate::api::{ApiEntry, ApiInput, ApiResult};
use crate::ctx::{BoxFuture, Ctx, Dep};
use crate::i18n::Text;
use crate::screen::{Icon, Screen, ScreenDef, ScreenMeta};
use serde::Serialize;
use std::sync::Arc;

/// Menu item.
#[derive(Clone, Debug, Serialize)]
pub struct MenuItem {
    pub label: Text,
    pub screen: &'static str,
    /// The icon before the label in the side menu, set with [`Menu::icon`].
    pub icon: Option<Icon>,
    /// A short fixed text after the label, set with [`Menu::badge`].
    pub badge: Option<Text>,
    /// A number after the label, counted per request, set with [`Menu::count`].
    #[serde(skip)]
    pub count: Option<MenuCount>,
    /// Topics that make the browser count again, set with [`Menu::count_listens`].
    pub topics: Vec<&'static str>,
}

/// The action that counts a menu item's number: it takes `()` and returns a `u64`, and runs with
/// its permission, once per page and again when one of the item's topics is named.
#[derive(Clone, Copy)]
pub struct MenuCount {
    pub action: crate::action::ActionMeta,
    pub run: fn(crate::Ctx) -> crate::BoxFuture<crate::Result<u64>>,
}

impl std::fmt::Debug for MenuCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuCount")
            .field("action", &self.action.name)
            .finish()
    }
}

/// Menu group: a heading over items and, after them, nested groups (see [`Menu::sub`]).
#[derive(Clone, Debug, Serialize)]
pub struct MenuGroup {
    pub label: Text,
    /// The icon before the label. Top level groups have one; nested groups have none.
    pub icon: Option<Icon>,
    pub items: Vec<MenuItem>,
    /// Nested groups, drawn after the items, at most [`MENU_DEPTH`] levels below a top level
    /// group.
    pub groups: Vec<MenuGroup>,
    /// Whether the group stays open (no toggle button), set with [`Menu::open`]. Other groups
    /// are collapsible and start open only when they hold the current screen.
    pub open: bool,
}

/// How many levels of nested groups a top level menu group may hold.
pub const MENU_DEPTH: usize = 3;

impl MenuGroup {
    /// A group without items.
    fn new(label: Text, icon: Option<Icon>) -> Self {
        MenuGroup {
            label,
            icon,
            items: Vec::new(),
            groups: Vec::new(),
            open: false,
        }
    }

    /// The levels of nested groups below this one: 0 for a group without nested groups.
    pub fn depth(&self) -> usize {
        self.groups
            .iter()
            .map(|g| g.depth() + 1)
            .max()
            .unwrap_or(0)
    }

    /// The item of `screen` in this group or a nested one, with the labels of the groups from
    /// this one down to the item's own.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// #[screen(route = "/orders", public)]
    /// pub fn orders() -> Screen { Screen::new("Orders") }
    ///
    /// let feature = Feature::new("shop").screen(orders).menu(|m| {
    ///     m.group("Commerce", Icon::Cart)
    ///         .sub("Sales", |m| m.item("Orders", orders))
    /// });
    /// let (path, item) = feature.menu_groups()[0].find("orders").unwrap();
    /// assert_eq!(path.len(), 2);
    /// assert_eq!(item.screen, "orders");
    /// ```
    pub fn find(&self, screen: &str) -> Option<(Vec<&Text>, &MenuItem)> {
        if let Some(item) = self
            .items
            .iter()
            .find(|i| i.screen == screen)
        {
            return Some((vec![&self.label], item));
        }
        self.groups.iter().find_map(|g| {
            let (mut path, item) = g.find(screen)?;
            path.insert(0, &self.label);
            Some((path, item))
        })
    }
}

/// Menu builder.
#[derive(Debug, Default)]
pub struct Menu {
    groups: Vec<MenuGroup>,
    accordion: bool,
}

impl Menu {
    /// Starts a group; following `item` calls add to it.
    pub fn group(mut self, label: impl Into<Text>, icon: Icon) -> Self {
        self.groups
            .push(MenuGroup::new(label.into(), Some(icon)));
        self
    }

    /// Adds a nested group to the last group. `build` gets a menu whose `item`, `icon`, `open`
    /// and `sub` calls go to the nested group. Nested groups come after the items of their group
    /// and may nest [`MENU_DEPTH`] levels deep; `Feature::menu` panics on deeper menus.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// #[screen(route = "/orders", public)]
    /// pub fn orders() -> Screen { Screen::new("Orders") }
    /// #[screen(route = "/returns", public)]
    /// pub fn returns() -> Screen { Screen::new("Returns") }
    ///
    /// let feature = Feature::new("shop").screen(orders).screen(returns).menu(|m| {
    ///     m.group("Commerce", Icon::Cart)
    ///         .item("Orders", orders)
    ///         .sub("After sales", |m| m.item("Returns", returns))
    /// });
    /// assert_eq!(feature.menu_groups()[0].groups[0].items[0].screen, "returns");
    /// ```
    pub fn sub(mut self, label: impl Into<Text>, build: impl FnOnce(Menu) -> Menu) -> Self {
        let inner = build(Menu {
            groups: vec![MenuGroup::new(label.into(), None)],
            accordion: false,
        });
        let [group]: [MenuGroup; 1] = inner
            .groups
            .try_into()
            .expect("inside `sub`, add items with `item` and nested groups with `sub`; `group` starts a top level group and belongs outside");
        self.groups
            .last_mut()
            .expect("call `group` before `sub`")
            .groups
            .push(group);
        self
    }

    /// Keeps the last group open: its heading is plain text instead of a toggle button.
    pub fn open(mut self) -> Self {
        if let Some(group) = self.groups.last_mut() {
            group.open = true;
        }
        self
    }

    /// Makes the side menu an accordion: opening a group closes its open siblings. Set on the
    /// top level menu of any feature in the app shell, it applies to the whole side menu.
    pub fn accordion(mut self) -> Self {
        self.accordion = true;
        self
    }

    /// Adds a screen to the last group. A screen without a route cannot be added (compile error).
    pub fn item<S: ScreenDef>(mut self, label: impl Into<Text>, _screen: S) -> Self {
        const {
            assert!(
                S::META.route.is_some(),
                "a menu item must point to a screen that has a route"
            )
        };
        const {
            assert!(
                !crate::screen::route_has_segments(S::META.route),
                "a menu item cannot point to a screen whose route has path segments; link to it from a grid or a screen with `Effect::navigate`"
            )
        };
        let group = self
            .groups
            .last_mut()
            .expect("call `group` before `item`");
        group.items.push(MenuItem {
            label: label.into(),
            screen: S::META.name,
            icon: None,
            badge: None,
            count: None,
            topics: Vec::new(),
        });
        self
    }

    /// The icon of the last added item, drawn before its label in the side menu. When the items
    /// of a group have icons, the group's label is drawn without its own.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// #[screen(route = "/orders", public)]
    /// pub fn orders() -> Screen { Screen::new("Orders") }
    ///
    /// let feature = Feature::new("shop").screen(orders).menu(|m| {
    ///     m.group("Commerce", Icon::Cart)
    ///         .item("Orders", orders)
    ///         .icon(Icon::Cart)
    /// });
    /// assert!(feature.menu_groups()[0].items[0].icon.is_some());
    /// ```
    pub fn icon(mut self, icon: Icon) -> Self {
        if let Some(item) = self.last_item() {
            item.icon = Some(icon);
        }
        self
    }

    /// A short fixed text after the last item's label, such as "New" or "Beta".
    pub fn badge(mut self, text: impl Into<Text>) -> Self {
        if let Some(item) = self.last_item() {
            item.badge = Some(text.into());
        }
        self
    }

    /// A number after the last item's label, such as unread messages, counted by an action that
    /// takes `()` and returns `u64`. The page asks for every count of its menu in one request
    /// after it shows (`GET /_zeyn/menu/counts`), so a slow count never delays the page; users
    /// without the action's permission see no number; 0 shows nothing and more than 99 shows
    /// "99+".
    ///
    /// ```ignore
    /// #[action(permission = "mail.read")]
    /// pub async fn unread(_: (), mail: Mail, ctx: Ctx) -> Result<u64> { mail.unread(&ctx).await }
    /// m.group(t!("menu.mail"), Icon::Mail).item(t!("menu.inbox"), inbox).count(unread)
    /// ```
    pub fn count<A>(mut self, _action: A) -> Self
    where
        A: crate::action::Action<Input = (), Output = crate::Result<u64>>,
    {
        if let Some(item) = self.last_item() {
            item.count = Some(MenuCount {
                action: A::META,
                run: |ctx| Box::pin(async move { A::call(ctx, ()).await? }),
            });
        }
        self
    }

    /// Topics that make the page count the last item again, when an action names one with
    /// `ui.refresh(..)` or the live socket notifies it.
    pub fn count_listens(mut self, topics: impl crate::ui::Topics) -> Self {
        if let Some(item) = self.last_item() {
            item.topics = crate::ui::topic_list(topics);
        }
        self
    }

    fn last_item(&mut self) -> Option<&mut MenuItem> {
        self.groups
            .last_mut()
            .and_then(|g| g.items.last_mut())
    }
}

/// A route of a feature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Route {
    pub path: String,
    pub screen: &'static str,
}

/// Result of a guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardResult {
    Allow,
    /// Denied, with a message for the user.
    Deny(Text),
}

/// A check that runs on the server before any screen of a feature is opened and before any of its
/// grids or forms is called. Plain functions `fn(&Ctx) -> GuardResult` are guards.
pub trait Guard: Send + Sync + 'static {
    fn check(&self, ctx: &Ctx) -> GuardResult;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl<F> Guard for F
where
    F: Fn(&Ctx) -> GuardResult + Send + Sync + 'static,
{
    fn check(&self, ctx: &Ctx) -> GuardResult {
        self(ctx)
    }
}

/// Built in guard: the user must be signed in.
#[derive(Clone, Copy, Debug, Default)]
pub struct SignedIn;

impl Guard for SignedIn {
    fn check(&self, ctx: &Ctx) -> GuardResult {
        if ctx.identity().is_signed_in() {
            GuardResult::Allow
        } else {
            GuardResult::Deny(Text::key("zeyn.error.forbidden"))
        }
    }

    fn name(&self) -> &'static str {
        "SignedIn"
    }
}

/// A registered screen with type erased params.
#[derive(Clone, Copy)]
pub struct ScreenEntry {
    pub meta: ScreenMeta,
    pub deps: fn() -> Vec<Dep>,
    pub build: fn(Ctx, serde_json::Value) -> BoxFuture<Result<Screen>>,
    /// Reads a path segment of the route as a params field; see [`ScreenDef::path_value`].
    pub path_value: fn(&str, &str) -> Option<serde_json::Value>,
}

impl ScreenEntry {
    /// The entry of a screen, as `Feature::screen` registers it.
    pub fn of<S: ScreenDef>() -> Self {
        ScreenEntry {
            meta: S::META,
            deps: S::deps,
            build: build_erased::<S>,
            path_value: S::path_value,
        }
    }
}

fn build_erased<S: ScreenDef>(ctx: Ctx, params: serde_json::Value) -> BoxFuture<Result<Screen>> {
    match read_params::<S::Params>(params) {
        Some(p) => S::build(ctx, p),
        None => Box::pin(async { Err(Error::NotFound) }),
    }
}

/// Reads screen params. A screen opened without any (a menu link, a plain address) gets `null`;
/// when its params do not read from `null`, they are read from `{}`, so a `#[params]` struct
/// whose fields are all `Option` opens with every field `None`.
pub fn read_params<P: serde::de::DeserializeOwned>(params: serde_json::Value) -> Option<P> {
    let none = params.is_null();
    match serde_json::from_value(params) {
        Ok(p) => Some(p),
        Err(_) if none => {
            serde_json::from_value(serde_json::Value::Object(Default::default())).ok()
        }
        Err(_) => None,
    }
}

/// A feature: route prefix, guards, screens and menu.
///
/// Example:
/// ```ignore
/// Feature::new("stock").route_prefix("/stock").guard(SignedIn).screen(product_list)
/// ```
pub struct Feature {
    name: &'static str,
    prefix: String,
    screens: Vec<ScreenEntry>,
    guards: Vec<Arc<dyn Guard>>,
    menu: Vec<MenuGroup>,
    menu_accordion: bool,
    apis: Vec<ApiEntry>,
    media: Vec<crate::media::MediaEntry>,
    commands: Vec<crate::command::Command>,
    searches: Vec<crate::command::Search>,
    sitemap: Option<crate::seo::Sitemap>,
    site: Option<crate::site::SiteShell>,
    /// Screens of this feature that open in the sign in shell while the others keep theirs.
    auth: Option<(crate::site::SiteShell, Vec<&'static str>)>,
    #[cfg(feature = "server")]
    files: Vec<crate::files::FileEntry>,
    #[cfg(feature = "server")]
    scanner: Option<Arc<dyn crate::files::FileScanner>>,
}

impl Feature {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            prefix: String::new(),
            screens: Vec::new(),
            guards: Vec::new(),
            menu: Vec::new(),
            menu_accordion: false,
            apis: Vec::new(),
            commands: Vec::new(),
            searches: Vec::new(),
            sitemap: None,
            media: Vec::new(),
            site: None,
            auth: None,
            #[cfg(feature = "server")]
            files: Vec::new(),
            #[cfg(feature = "server")]
            scanner: None,
        }
    }

    /// Registers a command: the command palette lists it and its shortcut works on every page of
    /// the app, for users who pass the feature's guards and may run it. See [`crate::Command`].
    pub fn command(mut self, command: crate::command::Command) -> Self {
        self.commands.push(command);
        self
    }

    pub fn commands(&self) -> &[crate::command::Command] {
        &self.commands
    }

    /// Adds a server search to the command palette: as the user types, the palette calls the
    /// action with the query (after the feature's guards and the action's permission) and lists
    /// the hits under the commands. Choosing a hit opens its page without a reload.
    pub fn search<A>(mut self, action: A) -> Self
    where
        A: crate::action::Action<
                Input = String,
                Output = crate::action::Result<Vec<crate::command::SearchHit>>,
            >,
    {
        self.searches
            .push(crate::command::Search::of(action));
        self
    }

    pub fn searches(&self) -> &[crate::command::Search] {
        &self.searches
    }

    /// Serves `/sitemap.xml` from a public action that lists the site's URLs part by part (see
    /// [`crate::seo::Sitemap`]). Above 50,000 URLs it becomes a sitemap index of numbered parts.
    /// One sitemap per app; a second one is ignored.
    pub fn sitemap<A>(mut self, action: A) -> Self
    where
        A: crate::action::Action<
                Input = crate::seo::SitemapPart,
                Output = crate::action::Result<Vec<crate::seo::SitemapUrl>>,
            >,
    {
        self.sitemap = Some(crate::seo::Sitemap::of(action));
        self
    }

    pub fn sitemap_source(&self) -> Option<&crate::seo::Sitemap> {
        self.sitemap.as_ref()
    }

    /// Publishes an action as `POST /api/v1/{feature}/{action}`. Callers need an `Authorization`
    /// header; access, guards, interceptors and audit apply as for screens.
    pub fn api<A>(mut self, _action: A) -> Self
    where
        A: Action,
        A::Input: ApiInput,
        A::Output: ApiResult,
    {
        self.apis.push(ApiEntry::of::<A>());
        self
    }

    pub fn apis(&self) -> &[ApiEntry] {
        &self.apis
    }

    /// Serves the files of a media source at `/_zeyn/media/{name}/{id}` to users with
    /// `permission`, after the feature's guards. `name` must be unique in the app and have the
    /// shape of a media id (letters, digits, `-`, `_`, `.`).
    pub fn media(
        mut self,
        name: &'static str,
        permission: &'static str,
        source: impl crate::media::MediaSource,
    ) -> Self {
        self.media
            .push(crate::media::MediaEntry {
                name,
                permission,
                source: Arc::new(source),
                public_cache: None,
            });
        self
    }

    /// A media source anyone may read, signed in or not, such as product images, logos and public downloads.
    /// Responses carry no cookie and `Cache-Control: public, max-age={max_age}` (at most
    /// [`MAX_PUBLIC_MEDIA_CACHE`](crate::media::MAX_PUBLIC_MEDIA_CACHE) seconds), so browsers and a
    /// CDN keep them; the entity tag of [`Media::file`](crate::Media::file) still lets them
    /// revalidate. The feature's guards and the interceptors still run. Keep private files in a
    /// source registered with [`Feature::media`].
    ///
    /// `open` must return the same bytes for every visitor: no per user or per language content,
    /// since shared caches keep one response for everyone. With a long `max_age`, give ids that
    /// change with the content (a content hash such as `poster-3f9a2c.jpg`), so a new file gets a
    /// new address instead of waiting for caches to expire. Reads of public sources do not count
    /// against `App::rate_limit`, because one page loads many images and many visitors can share
    /// one address; IP allow and deny rules still apply. Limit bandwidth abuse in a CDN or reverse
    /// proxy in front of the app.
    ///
    /// ```ignore
    /// Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
    /// ```
    /// The browser then loads `/_zeyn/media/images/{id}`.
    pub fn public_media(
        mut self,
        name: &'static str,
        max_age: u32,
        source: impl crate::media::MediaSource,
    ) -> Self {
        self.media
            .push(crate::media::MediaEntry {
                name,
                permission: "",
                source: Arc::new(source),
                public_cache: Some(max_age.min(crate::media::MAX_PUBLIC_MEDIA_CACHE)),
            });
        self
    }

    pub fn media_sources(&self) -> &[crate::media::MediaEntry] {
        &self.media
    }

    /// Keeps uploads for fields with these `rules` in `store`, served at
    /// `/_zeyn/files/{rules.store()}`. Uploading and downloading need `access` and the
    /// feature's guards: a permission name for both, or
    /// [`FileAccess::read`](crate::files::FileAccess::read) with `.write(..)` for a separate
    /// upload permission. The store name must be unique in the app.
    ///
    /// ```ignore
    /// Feature::new("crm").files(CONTRACTS, "crm.contract.write", DiskStore::new("data/contracts"))
    /// ```
    #[cfg(feature = "server")]
    pub fn files(
        mut self,
        rules: crate::files::FileRules,
        access: impl Into<crate::files::FileAccess>,
        store: impl crate::files::FileStore,
    ) -> Self {
        self.files
            .push(crate::files::FileEntry {
                rules,
                access: access.into(),
                store: Arc::new(store),
            });
        self
    }

    #[cfg(feature = "server")]
    pub fn file_stores(&self) -> &[crate::files::FileEntry] {
        &self.files
    }

    /// Checks every file uploaded to this feature's stores before it is accepted, such as with a
    /// virus scanner. A refused file is deleted and the upload fails.
    #[cfg(feature = "server")]
    pub fn scan_files(mut self, scanner: impl crate::files::FileScanner) -> Self {
        self.scanner = Some(Arc::new(scanner));
        self
    }

    #[cfg(feature = "server")]
    pub fn file_scanner(&self) -> Option<&Arc<dyn crate::files::FileScanner>> {
        self.scanner.as_ref()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Prefix of all routes of the feature, such as `/stock`.
    pub fn route_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix
            .into()
            .trim_end_matches('/')
            .to_string();
        self
    }

    /// Adds a guard that every screen and call of the feature must pass.
    pub fn guard(mut self, guard: impl Guard) -> Self {
        self.guards.push(Arc::new(guard));
        self
    }

    /// Registers a screen.
    pub fn screen<S: ScreenDef>(mut self, _screen: S) -> Self {
        self.screens
            .push(ScreenEntry::of::<S>());
        self
    }

    /// The feature's menu. Panics when a group nests deeper than [`MENU_DEPTH`] levels.
    pub fn menu(mut self, build: impl FnOnce(Menu) -> Menu) -> Self {
        let menu = build(Menu::default());
        for g in &menu.groups {
            let depth = g.depth();
            assert!(
                depth <= MENU_DEPTH,
                "menu group `{}` of feature `{}` nests groups {depth} levels deep; the side menu allows at most {MENU_DEPTH} levels of `sub` below a `group`",
                g.label.english(),
                self.name,
            );
        }
        self.menu = menu.groups;
        self.menu_accordion = menu.accordion;
        self
    }

    /// Whether the feature's menu asked for an accordion side menu ([`Menu::accordion`]).
    pub fn menu_accordion(&self) -> bool {
        self.menu_accordion
    }

    /// Opens the feature's screens in the site shell (a top bar with a horizontal menu built from
    /// this feature's menu, and a footer) instead of the app shell's side menu. See
    /// [`crate::Shell`]. Screens of other features keep their own shell.
    pub fn shell(mut self, shell: crate::site::SiteShell) -> Self {
        self.site = Some(shell);
        self
    }

    /// The site shell, when the feature chose it.
    pub fn site_shell(&self) -> Option<&crate::site::SiteShell> {
        self.site.as_ref()
    }

    /// Opens the named screens of this feature in `shell` (a [`crate::Shell::auth`]); the others
    /// keep the feature's shell. For the sign in screens of a feature that also has work screens.
    #[cfg(feature = "accounts")]
    pub(crate) fn auth_screens(
        mut self,
        shell: crate::site::SiteShell,
        screens: Vec<&'static str>,
    ) -> Self {
        self.auth = Some((shell, screens));
        self
    }

    /// The shell a screen of this feature opens in: its sign in shell, the feature's site shell,
    /// or `None` for the app shell.
    pub(crate) fn shell_of(&self, screen: &str) -> Option<&crate::site::SiteShell> {
        match &self.auth {
            Some((shell, names)) if names.contains(&screen) => Some(shell),
            _ => self.site.as_ref(),
        }
    }

    pub fn menu_groups(&self) -> &[MenuGroup] {
        &self.menu
    }

    pub fn screens(&self) -> &[ScreenEntry] {
        &self.screens
    }

    pub fn guards(&self) -> &[Arc<dyn Guard>] {
        &self.guards
    }

    /// Runs the guards in order; the first denial wins.
    pub fn check_guards(&self, ctx: &Ctx) -> Result<()> {
        for g in &self.guards {
            if let GuardResult::Deny(reason) = g.check(ctx) {
                return Err(Error::Denied(reason));
            }
        }
        Ok(())
    }

    /// Full path of a screen of this feature, if it has a route.
    pub fn path_of(&self, meta: &ScreenMeta) -> Option<String> {
        meta.route
            .map(|r| format!("{}{r}", self.prefix))
    }

    /// Full paths of the screens that have a route.
    pub fn routes(&self) -> Vec<Route> {
        self.screens
            .iter()
            .filter_map(|s| {
                self.path_of(&s.meta).map(|path| Route {
                    path,
                    screen: s.meta.name,
                })
            })
            .collect()
    }

    /// Checks structure rules: no duplicate routes or screens, every menu item points to a
    /// registered screen with a route, command ids and shortcuts are usable and not used twice.
    pub fn check(&self) -> std::result::Result<(), Vec<String>> {
        let mut problems = Vec::new();
        let routes = self.routes();
        for (i, r) in routes.iter().enumerate() {
            if routes[..i]
                .iter()
                .any(|o| o.path == r.path)
            {
                problems.push(format!(
                    "route `{}` is used by more than one screen",
                    r.path
                ));
            }
        }
        for (i, s) in self.screens.iter().enumerate() {
            if self.screens[..i]
                .iter()
                .any(|o| o.meta.name == s.meta.name)
            {
                problems.push(format!("screen `{}` is registered twice", s.meta.name));
            }
        }
        for (i, m) in self.media.iter().enumerate() {
            if !crate::media::valid_media_id(m.name) {
                problems.push(format!(
                    "media source `{}` must use only letters, digits, `-`, `_` and `.`",
                    m.name
                ));
            }
            if self.media[..i]
                .iter()
                .any(|o| o.name == m.name)
            {
                problems.push(format!("media source `{}` is registered twice", m.name));
            }
        }
        for item in self.menu.iter().flat_map(|g| &g.items) {
            match self
                .screens
                .iter()
                .find(|s| s.meta.name == item.screen)
            {
                Some(s) if s.meta.route.is_none() => problems.push(format!(
                    "menu item `{}` points to screen `{}`, which has no route",
                    item.label.english(),
                    item.screen
                )),
                Some(_) => {}
                None => problems.push(format!(
                    "menu item `{}` points to screen `{}`, which is not registered in feature `{}`",
                    item.label.english(),
                    item.screen,
                    self.name
                )),
            }
        }
        let commands: Vec<(&str, &crate::command::Command)> = self
            .commands
            .iter()
            .map(|c| (self.name, c))
            .collect();
        problems.extend(crate::command::problems(
            &commands,
            true,
            crate::command::PALETTE_SHORTCUT,
        ));
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}
