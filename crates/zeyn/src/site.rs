//! The site shell: a page frame for public pages with a top bar, a horizontal menu and a footer.
//!
//! Screens open in the app shell (a side menu, for work screens) unless their feature chooses the
//! site shell with [`crate::Feature::shell`]. Both shells live in one app, and moving between
//! them goes through the browser runtime without a page reload.

use crate::i18n::Text;
use crate::screen::{Icon, Node, ScreenDef};

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

    /// The sign in shell: no menu and no top bar, the app's logo and name over one centered
    /// card (400 px), and the language and theme pickers at the bottom, so they stay reachable.
    /// `.split()` puts the form on one half and a brand panel on the other from 1024 px. Every
    /// sign in screen of `zeyn::accounts::ui` uses it; a custom sign in screen gets it with
    /// `Feature::shell(Shell::auth())`.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// #[screen(route = "/sign-in", public)]
    /// pub fn sign_in() -> Screen { Screen::new("Sign in") }
    ///
    /// let feature = Feature::new("entry")
    ///     .screen(sign_in)
    ///     .shell(Shell::auth().split());
    /// assert!(feature.site_shell().is_some());
    /// ```
    pub fn auth() -> SiteShell {
        SiteShell {
            auth: Some(AuthLayout::Centered),
            ..SiteShell::default()
        }
    }
}

/// How the sign in shell lays out its page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthLayout {
    /// One card in the middle of the page.
    Centered,
    /// The form on one half, a brand panel on the other.
    Split,
}

/// Settings of the site shell and the sign in shell. Built with [`Shell::site`] or
/// [`Shell::auth`].
#[derive(Default)]
pub struct SiteShell {
    pub(crate) header: SiteHeader,
    pub(crate) footer: SiteFooter,
    pub(crate) bottom: Vec<BottomNavItem>,
    pub(crate) auth: Option<AuthLayout>,
}

impl SiteShell {
    /// For [`Shell::auth`]: the form on the start half and a brand panel on the end half, from
    /// 1024 px; narrower screens show the centered card.
    pub fn split(mut self) -> Self {
        if self.auth.is_some() {
            self.auth = Some(AuthLayout::Split);
        }
        self
    }

    /// The top bar's slots and ready actions, see [`SiteHeader`].
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
        const {
            assert!(
                N <= 5,
                "bottom_nav holds at most five items; put the others in the feature's menu"
            )
        };
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
            .chain(self.bottom.iter().map(|b| b.screen))
    }

    /// Footer addresses that are not a path, `https:`, `http:` or `mailto:`.
    pub(crate) fn bad_urls(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.footer
            .columns
            .iter()
            .flat_map(|c| &c.links)
            .filter_map(|l| match l.target {
                FooterTarget::Url(u) if !safe_url(u) => Some(u),
                _ => None,
            })
    }
}

fn safe_url(url: &str) -> bool {
    (url.starts_with('/') && !url.starts_with("//"))
        || ["https://", "http://", "mailto:"]
            .iter()
            .any(|p| url.starts_with(p))
}

/// Where the center slot of the site header goes on phones (800 px wide and less).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CenterOnPhone {
    /// A full width row under the top bar. Needs no script.
    #[default]
    Row,
    /// Behind an icon button in the top bar that opens it as a row (W3C APG disclosure). The
    /// button is a magnifier named "Search" unless [`SiteHeader::center_button`] sets another.
    Icon,
}

/// The site shell's top bar, in slots that take any component, modeled on an app bar with
/// logical names: `start` comes before the brand, `center` sits between the menu and the end,
/// `end` closes the bar and `bottom` is a full width row under it, such as tabs or chips. Start
/// and end follow the writing direction, so they swap sides in right to left languages. Each
/// slot holds several nodes in order. Nothing is shown unless asked for.
///
/// `sign_in`, `language` and `theme` add ready components after the end slot. On phones they
/// fold into the menu with the menu's links; the slots stay in the bar, and the center slot goes
/// where [`SiteHeader::center_on_phone`] says.
///
/// Components in slots render with the user's language, permissions and masks like a screen's.
/// Those that load data through their own action (a [`crate::ui::SearchBox`], a
/// [`crate::ui::Tree`]) work there, checked against the feature's guards and the action's
/// permission; grids and forms belong in the screen.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::CenterOnPhone;
///
/// #[model]
/// pub struct Product {
///     pub name: String,
/// }
///
/// #[action(public)]
/// pub async fn search_products(q: DataQuery) -> Result<Page<Product>> {
///     Ok(Page::default())
/// }
///
/// #[screen(route = "/search", public)]
/// pub fn results(p: SearchParams) -> Screen { Screen::new("Results") }
///
/// #[screen(route = "/cart", public)]
/// pub fn cart() -> Screen { Screen::new("Cart") }
///
/// let shell = Shell::site().header(|h| {
///     h.center(SearchBox::of(search_products).results(results))
///         .center_on_phone(CenterOnPhone::Icon)
///         .end(LinkButton::new("Cart", cart, ()))
///         .language()
///         .theme()
/// });
/// ```
#[derive(Default)]
pub struct SiteHeader {
    pub(crate) sign_in: bool,
    pub(crate) language: bool,
    pub(crate) theme: bool,
    pub(crate) breadcrumb: bool,
    pub(crate) search: bool,
    pub(crate) account: bool,
    pub(crate) slots: [Vec<Node>; 4],
    pub(crate) center_on_phone: CenterOnPhone,
    pub(crate) center_button: Option<(Icon, Text)>,
}

/// Index of each slot in [`SiteHeader`]: the first number of a slot node's path.
pub(crate) const SLOT_NAMES: [&str; 4] = ["start", "center", "end", "bottom"];

impl SiteHeader {
    #[track_caller]
    fn put(mut self, slot: usize, node: impl Into<Node>) -> Self {
        self.slots[slot].push(Node::placed(node));
        self
    }

    /// A component before the brand, such as a back link or a logo.
    #[track_caller]
    pub fn start(self, node: impl Into<Node>) -> Self {
        self.put(0, node)
    }

    /// A component in the middle of the bar, such as a search box.
    #[track_caller]
    pub fn center(self, node: impl Into<Node>) -> Self {
        self.put(1, node)
    }

    /// A component at the end of the bar, such as a cart link.
    #[track_caller]
    pub fn end(self, node: impl Into<Node>) -> Self {
        self.put(2, node)
    }

    /// A full width row under the bar, such as chips or tabs.
    #[track_caller]
    pub fn bottom(self, node: impl Into<Node>) -> Self {
        self.put(3, node)
    }

    /// Where the center slot goes on phones: a row under the bar (the default) or behind an
    /// icon button.
    pub fn center_on_phone(mut self, place: CenterOnPhone) -> Self {
        self.center_on_phone = place;
        self
    }

    /// The icon and the name of the button that opens the center slot on phones, with
    /// [`CenterOnPhone::Icon`]. A magnifier named "Search" unless set.
    pub fn center_button(mut self, icon: Icon, label: impl Into<Text>) -> Self {
        self.center_button = Some((icon, label.into()));
        self
    }

    /// The slot node at `path`: the slot index, the node's index in it, then child indexes.
    pub(crate) fn node(&self, path: &crate::ui::NodePath) -> Option<&Node> {
        let mut at = path.indexes();
        let mut node = self
            .slots
            .get(at.next()?)?
            .get(at.next()?)?;
        for i in at {
            node = node.children().get(i)?;
        }
        Some(node)
    }

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

    /// Where the user is, at the start of the app shell's top bar: the menu group of the
    /// current screen, then the screen's title. `nav.js` keeps it up to date while the user moves
    /// between screens. The app shell only.
    pub fn breadcrumb(mut self) -> Self {
        self.breadcrumb = true;
        self
    }

    /// A search field that opens the command palette (also with its shortcut). The palette
    /// button leaves the side menu.
    pub fn search(mut self) -> Self {
        self.search = true;
        self
    }

    /// The account menu at the end of the bar: the user's initials, then a dialog with who is
    /// signed in, the theme, language and time zone pickers and the sign out link (a sign in
    /// link for visitors). The pickers leave the side menu.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// let app = App::new("Admin").header(|h| h.breadcrumb().search().account());
    /// ```
    pub fn account(mut self) -> Self {
        self.account = true;
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
    pub fn column(
        mut self,
        title: impl Into<Text>,
        build: impl FnOnce(FooterColumn) -> FooterColumn,
    ) -> Self {
        self.columns.push(build(FooterColumn {
            title: title.into(),
            links: Vec::new(),
        }));
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
        const {
            assert!(
                S::META.route.is_some(),
                "a footer link must point to a screen that has a route"
            )
        };
        self.links.push(FooterLink {
            label: label.into(),
            target: FooterTarget::Screen(S::META.name),
        });
        self
    }

    /// A link to an address outside the app, or a path of it: `/...`, `https://...`, `http://...`
    /// or `mailto:...`. Other schemes fail the app's structure check.
    pub fn url(mut self, label: impl Into<Text>, href: &'static str) -> Self {
        self.links.push(FooterLink {
            label: label.into(),
            target: FooterTarget::Url(href),
        });
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
        const {
            assert!(
                S::META.route.is_some(),
                "a bottom bar link must point to a screen that has a route"
            )
        };
        BottomNavItem {
            label: label.into(),
            screen: S::META.name,
            icon: None,
        }
    }

    /// An icon above the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}
