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
}

/// Menu group.
#[derive(Clone, Debug, Serialize)]
pub struct MenuGroup {
    pub label: Text,
    pub icon: Icon,
    pub items: Vec<MenuItem>,
}

/// Menu builder.
#[derive(Debug, Default)]
pub struct Menu {
    groups: Vec<MenuGroup>,
}

impl Menu {
    /// Starts a group; following `item` calls add to it.
    pub fn group(mut self, label: impl Into<Text>, icon: Icon) -> Self {
        self.groups.push(MenuGroup { label: label.into(), icon, items: Vec::new() });
        self
    }

    /// Adds a screen to the last group. A screen without a route cannot be added (compile error).
    pub fn item<S: ScreenDef>(mut self, label: impl Into<Text>, _screen: S) -> Self {
        const { assert!(S::META.route.is_some(), "a menu item must point to a screen that has a route") };
        const {
            assert!(
                !crate::screen::route_has_segments(S::META.route),
                "a menu item cannot point to a screen whose route has path segments; link to it from a grid or a screen with `Effect::navigate`"
            )
        };
        let group = self.groups.last_mut().expect("call `group` before `item`");
        group.items.push(MenuItem { label: label.into(), screen: S::META.name });
        self
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

fn build_erased<S: ScreenDef>(ctx: Ctx, params: serde_json::Value) -> BoxFuture<Result<Screen>> {
    match serde_json::from_value::<S::Params>(params) {
        Ok(p) => S::build(ctx, p),
        Err(_) => Box::pin(async { Err(Error::NotFound) }),
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
    apis: Vec<ApiEntry>,
    media: Vec<crate::media::MediaEntry>,
    commands: Vec<crate::command::Command>,
    searches: Vec<crate::command::Search>,
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
            apis: Vec::new(),
            commands: Vec::new(),
            searches: Vec::new(),
            media: Vec::new(),
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
        A: crate::action::Action<Input = String, Output = crate::action::Result<Vec<crate::command::SearchHit>>>,
    {
        self.searches.push(crate::command::Search::of(action));
        self
    }

    pub fn searches(&self) -> &[crate::command::Search] {
        &self.searches
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
        self.media.push(crate::media::MediaEntry { name, permission, source: Arc::new(source) });
        self
    }

    pub fn media_sources(&self) -> &[crate::media::MediaEntry] {
        &self.media
    }

    /// Keeps uploads for fields with these `rules` in `store`, served at
    /// `/_zeyn/files/{rules.store()}`. Uploading and downloading need `permission` and the
    /// feature's guards. The store name must be unique in the app.
    ///
    /// ```ignore
    /// Feature::new("crm").files(CONTRACTS, "crm.contract.write", DiskStore::new("data/contracts"))
    /// ```
    #[cfg(feature = "server")]
    pub fn files(
        mut self,
        rules: crate::files::FileRules,
        permission: &'static str,
        store: impl crate::files::FileStore,
    ) -> Self {
        self.files.push(crate::files::FileEntry { rules, permission, store: Arc::new(store) });
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
        self.prefix = prefix.into().trim_end_matches('/').to_string();
        self
    }

    /// Adds a guard that every screen and call of the feature must pass.
    pub fn guard(mut self, guard: impl Guard) -> Self {
        self.guards.push(Arc::new(guard));
        self
    }

    /// Registers a screen.
    pub fn screen<S: ScreenDef>(mut self, _screen: S) -> Self {
        self.screens.push(ScreenEntry { meta: S::META, deps: S::deps, build: build_erased::<S>, path_value: S::path_value });
        self
    }

    pub fn menu(mut self, build: impl FnOnce(Menu) -> Menu) -> Self {
        self.menu = build(Menu::default()).groups;
        self
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
        meta.route.map(|r| format!("{}{r}", self.prefix))
    }

    /// Full paths of the screens that have a route.
    pub fn routes(&self) -> Vec<Route> {
        self.screens
            .iter()
            .filter_map(|s| self.path_of(&s.meta).map(|path| Route { path, screen: s.meta.name }))
            .collect()
    }

    /// Checks structure rules: no duplicate routes or screens, every menu item points to a
    /// registered screen with a route, command ids and shortcuts are usable and not used twice.
    pub fn check(&self) -> std::result::Result<(), Vec<String>> {
        let mut problems = Vec::new();
        let routes = self.routes();
        for (i, r) in routes.iter().enumerate() {
            if routes[..i].iter().any(|o| o.path == r.path) {
                problems.push(format!("route `{}` is used by more than one screen", r.path));
            }
        }
        for (i, s) in self.screens.iter().enumerate() {
            if self.screens[..i].iter().any(|o| o.meta.name == s.meta.name) {
                problems.push(format!("screen `{}` is registered twice", s.meta.name));
            }
        }
        for (i, m) in self.media.iter().enumerate() {
            if !crate::media::valid_media_id(m.name) {
                problems.push(format!("media source `{}` must use only letters, digits, `-`, `_` and `.`", m.name));
            }
            if self.media[..i].iter().any(|o| o.name == m.name) {
                problems.push(format!("media source `{}` is registered twice", m.name));
            }
        }
        for item in self.menu.iter().flat_map(|g| &g.items) {
            match self.screens.iter().find(|s| s.meta.name == item.screen) {
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
        let commands: Vec<(&str, &crate::command::Command)> = self.commands.iter().map(|c| (self.name, c)).collect();
        problems.extend(crate::command::problems(&commands, true, crate::command::PALETTE_SHORTCUT));
        if problems.is_empty() { Ok(()) } else { Err(problems) }
    }
}
