//! The site shell: a page frame for public pages with a top bar, a horizontal menu and a footer.
//!
//! Screens open in the app shell (a side menu, for work screens) unless their feature chooses the
//! site shell with [`crate::Feature::shell`]. Both shells live in one app, and moving between
//! them goes through the browser runtime without a page reload.

use crate::i18n::Text;
use crate::screen::{Icon, ScreenDef};

/// Chooses the page frame of a feature's screens. The app shell is the default; call
/// [`Shell::site`] for public pages.
///
/// ```
/// use zeyn::prelude::*;
///
/// #[screen(route = "/", public)]
/// pub fn home() -> Screen { Screen::new("Welcome") }
///
/// #[screen(route = "/about", public)]
/// pub fn about() -> Screen { Screen::new("About") }
///
/// let shop = Feature::new("shop")
///     .screen(home)
///     .screen(about)
///     .menu(|m| m.group("Shop", Icon::Home).item("Home", home).item("About", about))
///     .shell(
///         Shell::site()
///             .header(|h| h.language().theme().sign_in())
///             .footer(|f| f.column("Company", |c| c.link("About", about).url("Source", "https://example.com"))
///                 .text("Made with Zeyn")),
///     );
/// assert!(shop.site_shell().is_some());
/// ```
pub struct Shell;

impl Shell {
    /// The site shell: brand, a horizontal menu from the feature's menu, a menu button on narrow
    /// screens, optional actions and a footer.
    pub fn site() -> SiteShell {
        SiteShell::default()
    }
}

/// Settings of the site shell. Built with [`Shell::site`].
#[derive(Default)]
pub struct SiteShell {
    pub(crate) header: SiteHeader,
    pub(crate) footer: SiteFooter,
    pub(crate) bottom: Vec<BottomNavItem>,
}

impl SiteShell {
    /// Actions in the top bar.
    pub fn header(mut self, build: impl FnOnce(SiteHeader) -> SiteHeader) -> Self {
        self.header = build(self.header);
        self
    }

    /// Link columns and a line of text at the bottom of every page.
    pub fn footer(mut self, build: impl FnOnce(SiteFooter) -> SiteFooter) -> Self {
        self.footer = build(self.footer);
        self
    }

    /// A bar of up to five links fixed to the bottom of the screen on phones (800 px wide and
    /// less), for the few places people go most; the top menu stays for the rest. The link of
    /// the page being shown is marked with `aria-current="page"`, links to screens the user may
    /// not open are left out, the bar keeps clear of the system's home indicator
    /// (`env(safe-area-inset-bottom)`), and the page gets room at its bottom so the bar hides
    /// nothing. More than five items do not compile.
    ///
    /// ```
    /// use zeyn::prelude::*;
    /// use zeyn::BottomNavItem;
    ///
    /// #[screen(route = "/", public)]
    /// pub fn home() -> Screen { Screen::new("Home") }
    ///
    /// #[screen(route = "/search", public)]
    /// pub fn search() -> Screen { Screen::new("Search") }
    ///
    /// let shell = Shell::site().bottom_nav([
    ///     BottomNavItem::new("Home", home).icon(Icon::Home),
    ///     BottomNavItem::new("Search", search).icon(Icon::Search),
    /// ]);
    /// ```
    ///
    /// Six items are refused:
    ///
    /// ```compile_fail
    /// use zeyn::prelude::*;
    /// use zeyn::BottomNavItem;
    /// #[screen(route = "/", public)]
    /// pub fn home() -> Screen { Screen::new("Home") }
    /// let i = || BottomNavItem::new("Home", home);
    /// let shell = Shell::site().bottom_nav([i(), i(), i(), i(), i(), i()]);
    /// ```
    pub fn bottom_nav<const N: usize>(mut self, items: [BottomNavItem; N]) -> Self {
        const { assert!(N <= 5, "bottom_nav holds at most five items; put the others in the feature's menu") };
        self.bottom = items.into();
        self
    }

    /// Screens the footer and the bottom bar link to, for the app's structure check.
    pub(crate) fn linked_screens(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.footer
            .columns
            .iter()
            .flat_map(|c| &c.links)
            .filter_map(|l| match l.target {
                FooterTarget::Screen(s) => Some(s),
                FooterTarget::Url(_) => None,
            })
            .chain(
                self.bottom
                    .iter()
                    .map(|b| b.screen),
            )
    }

    /// Footer addresses that are not a path, `https:`, `http:` or `mailto:`.
    pub(crate) fn bad_urls(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.footer.columns.iter().flat_map(|c| &c.links).filter_map(|l| match l.target {
            FooterTarget::Url(u) if !safe_url(u) => Some(u),
            _ => None,
        })
    }
}

fn safe_url(url: &str) -> bool {
    (url.starts_with('/') && !url.starts_with("//"))
        || ["https://", "http://", "mailto:"].iter().any(|p| url.starts_with(p))
}

/// Actions of the site shell's top bar. None is shown unless asked for.
#[derive(Default)]
pub struct SiteHeader {
    pub(crate) sign_in: bool,
    pub(crate) language: bool,
    pub(crate) theme: bool,
}

impl SiteHeader {
    /// A sign in link for visitors and a sign out link for signed in users, when the app has a
    /// sign in page (`App::login_url`, OIDC or accounts).
    pub fn sign_in(mut self) -> Self {
        self.sign_in = true;
        self
    }

    /// The language picker, when the app has more than one language.
    pub fn language(mut self) -> Self {
        self.language = true;
        self
    }

    /// The theme picker, when the app lets users pick a theme.
    pub fn theme(mut self) -> Self {
        self.theme = true;
        self
    }
}

/// Footer of the site shell.
#[derive(Default)]
pub struct SiteFooter {
    pub(crate) columns: Vec<FooterColumn>,
    pub(crate) text: Option<Text>,
}

impl SiteFooter {
    /// A column of links under a heading.
    pub fn column(mut self, title: impl Into<Text>, build: impl FnOnce(FooterColumn) -> FooterColumn) -> Self {
        self.columns.push(build(FooterColumn { title: title.into(), links: Vec::new() }));
        self
    }

    /// A line of text under the columns, such as a copyright note. Use a `t!` key so it is
    /// translated.
    pub fn text(mut self, text: impl Into<Text>) -> Self {
        self.text = Some(text.into());
        self
    }
}

pub(crate) enum FooterTarget {
    Screen(&'static str),
    Url(&'static str),
}

pub(crate) struct FooterLink {
    pub(crate) label: Text,
    pub(crate) target: FooterTarget,
}

/// One column of footer links.
pub struct FooterColumn {
    pub(crate) title: Text,
    pub(crate) links: Vec<FooterLink>,
}

impl FooterColumn {
    /// A link to a screen of the app. Users who may not open it do not see it. A screen without
    /// a route cannot be linked (compile error).
    pub fn link<S: ScreenDef>(mut self, label: impl Into<Text>, _screen: S) -> Self {
        const { assert!(S::META.route.is_some(), "a footer link must point to a screen that has a route") };
        self.links.push(FooterLink { label: label.into(), target: FooterTarget::Screen(S::META.name) });
        self
    }

    /// A link to an address outside the app, or a path of it: `/...`, `https://...`, `http://...`
    /// or `mailto:...`. Other schemes fail the app's structure check.
    pub fn url(mut self, label: impl Into<Text>, href: &'static str) -> Self {
        self.links.push(FooterLink { label: label.into(), target: FooterTarget::Url(href) });
        self
    }
}

/// One link of the site shell's bottom bar ([`SiteShell::bottom_nav`]): a label, a screen with a
/// route and an optional icon above the label.
pub struct BottomNavItem {
    pub(crate) label: Text,
    pub(crate) screen: &'static str,
    pub(crate) icon: Option<Icon>,
}

impl BottomNavItem {
    /// A link to a screen of the app. A screen without a route cannot be linked (compile error).
    pub fn new<S: ScreenDef>(label: impl Into<Text>, _screen: S) -> Self {
        const { assert!(S::META.route.is_some(), "a bottom bar link must point to a screen that has a route") };
        BottomNavItem { label: label.into(), screen: S::META.name, icon: None }
    }

    /// An icon above the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}
