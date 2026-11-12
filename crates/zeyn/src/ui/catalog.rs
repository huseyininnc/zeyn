//! Catalog components for public pages: [`CardGrid`] (the card view of a DataGrid), [`Rail`] (a
//! horizontal list of cards) and [`SearchBox`] (a search field with suggestions).
//!
//! All three load rows through an action that takes a [`DataQuery`] and returns a [`Page`], the
//! same action a DataGrid uses. Paging, sorting, free text search and permissions run on the
//! server; the browser never chooses a page size, a field or a filter on its own.

use super::embed::{Loading, Placeholder};
use super::layout::Card;
use super::list::ListItem;
use super::widgets::{Badge, screen_href};
use super::{DataSource, dom_id};
use crate::action::{ActionMeta, Cursor, DataQuery, Error, Page, PageRequest, Result, Sort};
use crate::ctx::{BoxFuture, Ctx};
use crate::grid::{FetchFn, Paging, fetch_via};
use crate::i18n::Text;
use crate::model::Model;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, GridSource, LoadedPage, Node, Params, ScreenDef};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Params of a search results screen: the text the user searched for. A screen opened without
/// params (from a menu) gets empty text.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::SearchParams;
///
/// #[screen(route = "/search", public)]
/// pub fn results(p: SearchParams) -> Screen {
///     Screen::new("Results").body(zeyn::ui::Alert::info(p.q))
/// }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SearchParams {
    pub q: String,
}

impl<'de> Deserialize<'de> for SearchParams {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            q: String,
        }
        Ok(Option::<Raw>::deserialize(d)?
            .map(|r| SearchParams { q: r.q })
            .unwrap_or_default())
    }
}

impl Params for SearchParams {}

/// One card of a [`CardGrid`] or [`Rail`]: an image, a title, an optional subtitle, text,
/// badges and progress bar, and the screen the card opens. Built from a row with
/// [`Card::image`].
///
/// ```
/// use zeyn::prelude::*;
/// let card = Card::image("/img/course-1.jpg", "Intro to baking")
///     .subtitle("Six lessons")
///     .text("12 min left")
///     .corner(Badge::new("New").tone(Tone::Success))
///     .badges([Badge::new("Beginner"), Badge::new("Video")])
///     .progress(0.4, "Watched")
///     .horizontal();
/// ```
pub struct ImageCard {
    image: String,
    title: Text,
    subtitle: Option<Text>,
    text: Option<Text>,
    alt: Text,
    badge: Option<Badge>,
    corner: Option<Badge>,
    badges: Vec<Badge>,
    progress: Option<(f32, Text)>,
    price: Option<super::Price>,
    horizontal: bool,
    link: Option<(&'static str, Json)>,
    preview: super::preview::PreviewTarget,
    loading: Loading,
}

impl Card {
    /// A card with an image and a title, for a [`CardGrid`] or [`Rail`]. The image is a path of
    /// the app (`/...`), an `https:` address or a `data:image/` URI; anything else is left out.
    /// Its alternative text is empty, because the title next to it names the card; give
    /// [`ImageCard::alt`] when the image says more than the title.
    pub fn image(image: impl Into<String>, title: impl Into<Text>) -> ImageCard {
        ImageCard {
            image: image.into(),
            title: title.into(),
            subtitle: None,
            text: None,
            alt: Text::from(""),
            badge: None,
            corner: None,
            badges: Vec::new(),
            progress: None,
            price: None,
            horizontal: false,
            link: None,
            preview: None,
            loading: Loading::default(),
        }
    }
}

impl ImageCard {
    /// A line under the title, such as a price.
    pub fn text(mut self, text: impl Into<Text>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Alternative text of the image, when it shows something the title does not say.
    pub fn alt(mut self, alt: impl Into<Text>) -> Self {
        self.alt = alt.into();
        self
    }

    /// A badge on the image, such as "New", at its starting corner.
    pub fn badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
        self
    }

    /// A second line next to the title, such as an author or a category, in a quieter color.
    pub fn subtitle(mut self, subtitle: impl Into<Text>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A badge over the image at its ending corner (top right, top left in right to left
    /// pages), such as a count or a status.
    pub fn corner(mut self, badge: Badge) -> Self {
        self.corner = Some(badge);
        self
    }

    /// Badges in a row under the text, such as tags. Several calls add to the row.
    pub fn badges(mut self, badges: impl IntoIterator<Item = Badge>) -> Self {
        self.badges.extend(badges);
        self
    }

    /// A thin progress bar along the bottom of the image, such as how much of a course is done.
    /// `fraction` runs from 0 to 1 (values outside are clamped, `NaN` is 0); `label` names it for
    /// screen readers, which also hear the percentage.
    pub fn progress(mut self, fraction: f32, label: impl Into<Text>) -> Self {
        let f = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        self.progress = Some((f, label.into()));
        self
    }

    /// The price under the text, such as the current and the old price of a product. It is the
    /// same [`super::Price`] a product page shows, inside the card's link, so screen readers hear
    /// the card's title and the price sentence together; it needs no script.
    pub fn price(mut self, price: super::Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Lays the card out in a row: a small image at the start and the text beside it, for lists
    /// of compact cards. The card grid or rail gives such cards wider columns.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    /// The screen the card opens. The whole card is one link, loaded without a page reload. The
    /// link is the screen's canonical address when it has a route (`/items/{slug}`), so search
    /// engines and visitors without JavaScript follow it directly. Params of the wrong type do
    /// not compile.
    pub fn link<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.link = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }

    /// A preview of `screen` in a panel next to the card: after about half a second of hover
    /// with a mouse, or at once on keyboard focus. The screen loads only then, through the same
    /// access checks as a window, and is kept for the page's life. Escape closes the panel, the
    /// pointer can move onto it, frames and media inside are left out, and touch screens show no
    /// preview (a tap opens the link). Needs [`ImageCard::link`]. Params of the wrong type do not
    /// compile.
    pub fn preview<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.preview = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }

    /// What the image shows while it loads, when the card grid's [`CardGrid::placeholder`] does
    /// not fit this card. See [`Placeholder`].
    pub fn placeholder(mut self, placeholder: Placeholder) -> Self {
        self.loading.placeholder = Some(placeholder);
        self
    }

    /// A small, low resolution file of the image, shown at once while the full one loads. It
    /// follows the app's `img-src` rules, as [`crate::ui::Image::low`].
    pub fn low(mut self, src: impl Into<String>) -> Self {
        self.loading.low = Some(src.into());
        self
    }

    /// Another file tried once when the image fails, before the alt text or title is shown with
    /// an icon. It follows the app's `img-src` rules.
    pub fn fallback(mut self, src: impl Into<String>) -> Self {
        self.loading.fallback = Some(src.into());
        self
    }

    /// Loads this card's image at once with `fetchpriority="high"` instead of lazily. Prefer
    /// [`CardGrid::priority`], which marks the first cards of the first page.
    pub fn priority(mut self) -> Self {
        self.loading.priority = true;
        self
    }

    fn html(
        mut self,
        out: &mut String,
        r: &Render,
        size: (u32, u32),
        placeholder: Option<&Placeholder>,
        priority: bool,
    ) {
        if self.loading.placeholder.is_none() {
            self.loading.placeholder = placeholder.cloned();
        }
        self.loading.priority |= priority;
        let wrap = self.loading.wraps() && safe_image(&self.image);
        let img = if safe_image(&self.image) {
            format!(
                r#"{}<img class="z-poster-img" src="{}" alt="{}" width="{}" height="{}"{} decoding="async" draggable="false">{}"#,
                if wrap {
                    self.loading.layers(r, true)
                } else {
                    String::new()
                },
                escape(&self.image),
                r.text(&self.alt),
                size.0,
                size.1,
                self.loading.img_attrs(r, true),
                self.loading.error(r),
            )
        } else {
            format!(
                r#"<span class="z-poster-img" style="aspect-ratio:{} / {}"></span>"#,
                size.0, size.1
            )
        };
        let badge = self
            .badge
            .as_ref()
            .map(|b| b.html(r))
            .unwrap_or_default();
        let corner = self
            .corner
            .as_ref()
            .map(|b| format!(r#"<span class="z-poster-corner">{}</span>"#, b.html(r)))
            .unwrap_or_default();
        let progress = self
            .progress
            .as_ref()
            .map(|(f, label)| {
                let pct = (f * 100.0).round() as u32;
                format!(
                    r#"<span class="z-poster-progress" role="progressbar" aria-label="{}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{pct}" aria-valuetext="{pct}%"><span class="z-poster-progress-bar" style="--z-progress:{pct}%"></span></span>"#,
                    r.text(label)
                )
            })
            .unwrap_or_default();
        let line = |t: &Option<Text>, class: &str| {
            t.as_ref()
                .map(|t| format!(r#"<span class="{class}">{}</span>"#, r.text(t)))
                .unwrap_or_default()
        };
        let subtitle = line(&self.subtitle, "z-poster-subtitle");
        let text = line(&self.text, "z-poster-text");
        let price = self
            .price
            .as_ref()
            .map(|p| p.html(r))
            .unwrap_or_default();
        let badges = if self.badges.is_empty() {
            String::new()
        } else {
            let all: String = self
                .badges
                .iter()
                .map(|b| b.html(r))
                .collect();
            format!(r#"<span class="z-poster-badges">{all}</span>"#)
        };
        let media = if wrap {
            format!(
                r#"<span class="z-poster-media{}" style="{}">"#,
                self.loading.host_class(),
                self.loading.host_style(size)
            )
        } else {
            String::from(r#"<span class="z-poster-media">"#)
        };
        let body = format!(
            r#"{media}{img}{badge}{corner}{progress}</span><span class="z-poster-body"><span class="z-poster-title">{}</span>{subtitle}{text}{price}{badges}</span>"#,
            r.text(&self.title)
        );
        let class = if self.horizontal {
            "z-poster z-poster-h"
        } else {
            "z-poster"
        };
        match &self.link {
            Some((screen, params)) => out.push_str(&format!(
                r#"<li class="{class}"><a class="z-poster-link" href="{}" data-zeyn-base-href{}>{body}</a></li>"#,
                screen_href(r, screen, params),
                super::preview::preview_attr(&self.preview)
            )),
            None => out.push_str(&format!(
                r#"<li class="{class}"><div class="z-poster-link">{body}</div></li>"#
            )),
        }
    }
}

/// Whether an image address may go in a card or list row: a path of the app, an `https:`
/// address or a `data:image/` URI.
pub(crate) fn safe_image(src: &str) -> bool {
    src.starts_with('/') && !src.starts_with("//")
        || src.starts_with("https://")
        || src.starts_with("data:image/")
}

type CardFn<E> = Box<dyn Fn(&E) -> ImageCard + Send + Sync>;
type ItemFn<E> = Box<dyn Fn(&E) -> ListItem + Send + Sync>;

/// Turns away rows of a model with `#[mask]` fields the user may not reveal. Cards and
/// suggestions are built by app closures that see the raw row, so they could not keep a masked
/// value masked.
pub(crate) fn check_masks<E: Model>(ctx: &Ctx) -> Result<()> {
    if E::fields()
        .iter()
        .any(|f| !crate::privacy::can_unmask(Some(ctx), f))
    {
        Err(Error::Forbidden)
    } else {
        Ok(())
    }
}

/// The card view of a DataGrid: rows of an action shown as cards in a responsive grid, with
/// numbered pages or "load more" paging and an optional sort picker. Paging, sorting and the
/// search text run on the server through the action and its [`DataQuery`]; the server keeps the
/// page size and accepts only the sort orders offered here. Reading needs the action's access,
/// as for a DataGrid.
///
/// Cards are built by a closure that sees the raw row, so a model with `#[mask]` fields is only
/// shown to users who may reveal them; everyone else gets `Forbidden`.
///
/// ```ignore
/// CardGrid::of(search_products)
///     .title(t!("shop.products"))
///     .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()).text(p.price.to_string())
///         .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
///         .link(product_page, ProductParams { id: p.id }))
///     .sort_option(t!("shop.by_name"), Product::name, false)
///     .paging(Paging::infinite(24))
/// ```
pub struct CardGrid<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    card: Option<CardFn<E>>,
    item: Option<ItemFn<E>>,
    title: Option<Text>,
    paging: Paging,
    sorts: Vec<(Text, Sort)>,
    search: String,
    image: (u32, u32),
    rail: bool,
    list: bool,
    scrollbar: bool,
    scope: Vec<crate::action::Filter>,
    builder: Option<BuildFn<E>>,
    separator: Option<Node>,
    row_height: Option<u32>,
    placeholder: Option<Placeholder>,
    priority: usize,
    empty: Option<super::EmptyState>,
    empty_filtered: Option<super::EmptyState>,
}

type BuildFn<E> = Box<dyn Fn(&E) -> Node + Send + Sync>;

/// A horizontal list of cards that scrolls sideways, with previous and next buttons and scroll
/// snapping. It loads the next batch when the user scrolls near its end. Built like a
/// [`CardGrid`], which it is, laid out in one row.
///
/// The scrollbar is hidden unless [`CardGrid::scrollbar`] keeps it; touch, trackpad and
/// Shift+wheel still scroll. A mouse can press and drag the rail sideways: after a move of more
/// than 5 pixels the click on release is cancelled, so no card opens, and the rail snaps to the
/// nearest card. Touch keeps the browser's own scrolling. Without JavaScript the buttons are
/// hidden and the rail scrolls natively.
///
/// Keyboard: the cards are links in the tab order and scroll into view as they get focus; the
/// previous and next buttons scroll by a view's width and are disabled at the ends. They are
/// always there, because the hidden scrollbar is no way to scroll for a keyboard.
///
/// ```ignore
/// Rail::of(search_products).title(t!("shop.new_arrivals")).card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
/// ```
pub struct Rail;

impl Rail {
    /// A rail that loads its cards from the given action, 12 at a time.
    #[allow(clippy::new_ret_no_self)]
    pub fn of<A, E>(source: A) -> CardGrid<E>
    where
        A: crate::action::Action<Input = DataQuery, Output = Result<Page<E>>>,
        E: Model,
    {
        let mut grid = CardGrid::of(source);
        grid.rail = true;
        grid.paging = Paging::infinite(12);
        grid
    }
}

impl<E: Model> CardGrid<E> {
    /// Cards of the rows of the given action, 24 per page with a "load more" button.
    pub fn of<A>(_source: A) -> Self
    where
        A: crate::action::Action<Input = DataQuery, Output = Result<Page<E>>>,
    {
        Self {
            action: A::META,
            fetch: fetch_via::<A, E>,
            card: None,
            item: None,
            title: None,
            paging: Paging::infinite(24),
            sorts: Vec::new(),
            search: String::new(),
            image: (300, 400),
            rail: false,
            list: false,
            scrollbar: false,
            scope: Vec::new(),
            builder: None,
            separator: None,
            row_height: None,
            placeholder: None,
            priority: 0,
            empty: None,
            empty_filtered: None,
        }
    }

    /// What shows when there are no rows, in place of the built in "Nothing here": a title, a
    /// text and an action, such as "No products yet" with "Add a product". With
    /// [`CardGrid::empty_filtered`] it shows only when there is no search text.
    pub fn empty(mut self, state: super::EmptyState) -> Self {
        self.empty = Some(state);
        self
    }

    /// What shows when a search leaves no rows, such as "Nothing matches" with a hint.
    pub fn empty_filtered(mut self, state: super::EmptyState) -> Self {
        self.empty_filtered = Some(state);
        self
    }

    /// The empty state's HTML: the app's own for the case at hand, or the built in text.
    fn empty_html(&self, r: &Render, fallback: String) -> String {
        let state = if self.search.is_empty() {
            self.empty.as_ref()
        } else {
            self.empty_filtered
                .as_ref()
                .or(self.empty.as_ref())
        };
        match state {
            Some(s) => {
                let mut out = String::new();
                s.render(&mut out, r);
                out
            }
            None => fallback,
        }
    }

    /// Builds each row as any node, an escape hatch next to [`CardGrid::card`] and
    /// [`CardGrid::item`], which stay the default: they give links, images and screen reader
    /// text for free. The row's `<li>` carries the model key (`data-zeyn-key`), so a refresh
    /// keeps focus and open details on the same record. Built rows are for showing: the nodes in
    /// them are not addressed by node paths, so put grids, forms and lookups elsewhere. With
    /// `Paging::virtual_scroll()` on a list, rows must say their height with
    /// [`CardGrid::row_height`]; `App::check_page` reports a missing one.
    ///
    /// ```ignore
    /// List::of(search_people)
    ///     .builder(|p: &Person| Row::new().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    ///     .separator(Paragraph::new("·"))
    /// ```
    pub fn builder<N: Into<Node>>(
        mut self,
        build: impl Fn(&E) -> N + Send + Sync + 'static,
    ) -> Self {
        self.builder = Some(Box::new(move |row| build(row).into()));
        self
    }

    /// A node shown between two rows, such as a divider or a dot. It is hidden from screen
    /// readers, which count the rows themselves. Not used with virtual scrolling, where every
    /// row has the same height.
    pub fn separator(mut self, node: impl Into<Node>) -> Self {
        self.separator = Some(node.into());
        self
    }

    /// The fixed height of every row in pixels, for a list with `Paging::virtual_scroll()`:
    /// only the rows in view are in the page, placed by this height, and each carries
    /// `aria-posinset` and `aria-setsize`. There is no height per row; content that does not fit
    /// is cut. Required with [`CardGrid::builder`], at least 20.
    pub fn row_height(mut self, px: u32) -> Self {
        self.row_height = Some(px.max(20));
        self
    }

    /// The row height and block size of a virtual list, or `None` when it is not one. A list
    /// built with a builder and no row height falls back to loading more rows.
    fn virtual_rows(&self) -> Option<(u32, u32)> {
        match self.paging {
            Paging::Virtual { row_height, block } if self.list && !self.rail => {
                let height = match (self.row_height, &self.builder) {
                    (Some(h), _) => h,
                    (None, None) => row_height,
                    (None, Some(_)) => return None,
                };
                Some((height, block))
            }
            _ => None,
        }
    }

    /// Keeps the scrollbar of a [`Rail`] visible (thin). Rails hide it by default; card grids and
    /// lists do not scroll sideways, so it changes nothing for them.
    pub fn scrollbar(mut self, show: bool) -> Self {
        self.scrollbar = show;
        self
    }

    /// Lays the rows out as a [`super::List`]: see [`super::List::of`].
    pub(crate) fn into_list(mut self) -> Self {
        self.list = true;
        self.paging = Paging::infinite(20);
        self
    }

    /// How a row becomes a list row, for a list made with [`super::List::of`]. Card grids and
    /// rails use [`CardGrid::card`] instead.
    pub fn item(mut self, build: impl Fn(&E) -> ListItem + Send + Sync + 'static) -> Self {
        self.item = Some(Box::new(build));
        self
    }

    /// How a row becomes a card.
    pub fn card(mut self, build: impl Fn(&E) -> ImageCard + Send + Sync + 'static) -> Self {
        self.card = Some(Box::new(build));
        self
    }

    /// A heading over the cards, which also names the list for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// `Paging::infinite(n)` (the default, 24) loads `n` more with a button that also fires when
    /// it scrolls into view; `Paging::pages(n)` shows numbered pages with previous and next.
    /// Other kinds of paging work as `infinite`.
    pub fn paging(mut self, paging: Paging) -> Self {
        self.paging = paging;
        self
    }

    /// A sort order users can pick. The first one is the order the cards start in.
    pub fn sort_option<T>(
        mut self,
        label: impl Into<Text>,
        field: crate::model::Field<E, T>,
        desc: bool,
    ) -> Self {
        self.sorts.push((
            label.into(),
            Sort {
                field: field.meta().name.to_string(),
                desc,
            },
        ));
        self
    }

    /// Keeps only the rows whose `field` equals `value`, on every request, like
    /// [`crate::DataGrid::scope`]: `.scope(Article::author_id, author.id)` for the articles of one
    /// author. Client filters are dropped, so the client can neither change nor remove it. Text
    /// compares ignoring case.
    pub fn scope<T: crate::value::FieldType>(
        mut self,
        field: crate::model::Field<E, T>,
        value: T,
    ) -> Self {
        self.scope.push(crate::grid::key_filter(
            field.meta(),
            crate::grid::value_key(&value.to_value()).as_deref(),
        ));
        self
    }

    /// Keeps only the rows whose `field` is one of `values`, on every request, like
    /// [`CardGrid::scope`]: `.scope_any(Product::color, p.colors.clone())` for the colors a facet
    /// panel chose. No values keep every row.
    pub fn scope_any<T: crate::value::FieldType>(
        mut self,
        field: crate::model::Field<E, T>,
        values: impl IntoIterator<Item = T>,
    ) -> Self {
        self.scope
            .extend(crate::grid::any_filter(field.meta(), values));
        self
    }

    /// Keeps only the rows whose `field` is between `from` and `to`, both inclusive, on every
    /// request, like [`CardGrid::scope`]; `None` leaves that end open:
    /// `.scope_range(Product::price, p.min_price.map(Money::tl), None)`.
    pub fn scope_range<T: crate::value::FieldType>(
        mut self,
        field: crate::model::Field<E, T>,
        from: Option<T>,
        to: Option<T>,
    ) -> Self {
        self.scope
            .extend(crate::grid::range_filter(field.meta(), from, to));
        self
    }

    /// Shows only rows that match this free text ([`DataQuery::q`]), such as the text of a
    /// search results screen.
    pub fn search(mut self, text: impl Into<String>) -> Self {
        self.search = text.into();
        self
    }

    /// Width and height of the card images in pixels, so the page does not jump while they load.
    /// 300 by 400 unless set.
    pub fn image_size(mut self, width: u32, height: u32) -> Self {
        self.image = (width.clamp(1, 4000), height.clamp(1, 4000));
        self
    }

    /// What every card image shows while it loads, unless the card sets its own. See
    /// [`Placeholder`].
    pub fn placeholder(mut self, placeholder: Placeholder) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    /// Loads the images of the first `n` cards at once with `fetchpriority="high"`, for the cards
    /// a visitor sees without scrolling; every other card image stays lazy. Only the page
    /// rendered with the screen counts, never pages loaded later. 0 unless set; at most 12.
    pub fn priority(mut self, n: usize) -> Self {
        self.priority = n.min(12);
        self
    }

    fn size(&self) -> u32 {
        match self.paging {
            Paging::Pages { size } => size,
            Paging::Infinite { batch } => batch,
            Paging::Virtual { block, .. } if self.virtual_rows().is_some() => block,
            _ => 24,
        }
    }

    fn pages(&self) -> bool {
        matches!(self.paging, Paging::Pages { .. }) && !self.rail
    }

    /// The query the server runs: the client's position and one of the offered sort orders; the
    /// page size and search text come from the server; client filters are dropped.
    fn allow(&self, query: &mut DataQuery) {
        let size = self.size();
        let offsets = self.pages() || self.virtual_rows().is_some();
        query.page = match &query.page {
            PageRequest::Offset { offset, .. } if offsets => PageRequest::Offset {
                offset: *offset,
                limit: size,
            },
            PageRequest::After { cursor, .. } if !offsets => PageRequest::After {
                cursor: cursor.clone(),
                limit: size,
            },
            _ if offsets => PageRequest::Offset {
                offset: 0,
                limit: size,
            },
            _ => PageRequest::After {
                cursor: None,
                limit: size,
            },
        };
        query
            .sort
            .retain(|s| self.sorts.iter().any(|(_, o)| o == s));
        query.sort.truncate(1);
        if query.sort.is_empty() {
            query.sort.extend(
                self.sorts
                    .first()
                    .map(|(_, s)| s.clone()),
            );
        }
        query.filters.clone_from(&self.scope);
        query.q = self.search.clone();
        query.with_total = offsets;
        query.aggregates.clear();
        query.group = None;
    }

    /// The rows of a page as `<li>`s. Built rows and the rows of a virtual list carry their model
    /// key in `data-zeyn-key`; in a virtual list also their position (`aria-posinset`) and the
    /// row count (`aria-setsize`). The cards of a card grid carry their key too, because a long
    /// grid keeps only the rows near the viewport in the page (`catalog.js`), so a refresh finds
    /// the focused card by its key, not by its position. Items of lists and cards of rails are
    /// links, found again by position.
    /// Separators go before each row; the stylesheet hides the one before the first row, so a
    /// batch appended later still gets one.
    fn cards(&self, page: &LoadedPage, r: &Render, first: bool) -> String {
        let mut out = String::new();
        let Some(rows) = page.items.downcast_ref::<Vec<E>>() else {
            return out;
        };
        let key = E::fields().iter().position(|f| f.key);
        let place = self
            .virtual_rows()
            .map(|_| page.total.unwrap_or(0));
        for (i, row) in rows.iter().enumerate() {
            if let (Some(sep), None) = (&self.separator, place) {
                out.push_str(r#"<li class="z-separator" aria-hidden="true">"#);
                sep.render(&mut out, r);
                out.push_str("</li>");
            }
            let start = out.len();
            match (&self.builder, &self.card, &self.item) {
                (Some(build), _, _) => {
                    let path = format!("{}.k{}", r.path, page.offset + i as u64);
                    out.push_str(r#"<li class="z-built">"#);
                    build(row).render(&mut out, &Render { path: &path, ..*r });
                    out.push_str("</li>");
                }
                (None, _, Some(item)) if self.list => item(row).html(&mut out, r),
                (None, Some(card), _) if !self.list => card(row).html(
                    &mut out,
                    r,
                    self.image,
                    self.placeholder.as_ref(),
                    first && i < self.priority,
                ),
                _ => continue,
            }
            if self.builder.is_none() && place.is_none() && (self.list || self.rail) {
                continue;
            }
            let mut attrs = key
                .and_then(|k| row.value_at(k))
                .and_then(|v| crate::grid::value_key(&v))
                .map(|k| format!(r#" data-zeyn-key="{}""#, escape(&k)))
                .unwrap_or_default();
            if let Some(total) = place {
                attrs.push_str(&format!(
                    r#" aria-posinset="{}" aria-setsize="{total}""#,
                    page.offset + i as u64 + 1
                ));
            }
            if out[start..].starts_with("<li") {
                out.insert_str(start + 3, &attrs);
            }
        }
        out
    }

    /// "Page 2 of 5" when the action counted its rows, "Page 2" when it did not.
    fn page_label(&self, page: &LoadedPage, t: &dyn Fn(&str) -> String) -> String {
        let size = u64::from(self.size().max(1));
        let at = (page.offset / size + 1).to_string();
        match page.total {
            Some(total) => t("page")
                .replace("{page}", &at)
                .replace("{pages}", &total.div_ceil(size).max(1).to_string()),
            None => t("page_only").replace("{page}", &at),
        }
    }

    /// A virtual list: a scroll area as tall as every row at the fixed height, holding the first
    /// block; `vlist.js` places the rows in view and fetches the other blocks by offset.
    fn render_virtual(
        &self,
        out: &mut String,
        r: &Render,
        page: Option<&LoadedPage>,
        (height, block): (u32, u32),
    ) {
        let id = dom_id(r, "cards");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.cards.{key}")));
        let total = page.and_then(|p| p.total).unwrap_or(0);
        let title = self
            .title
            .as_ref()
            .map(|t| r.text(t))
            .unwrap_or_else(|| t("list"));
        out.push_str(&format!(
            r#"<section class="z-cards z-cards-rows z-vlist" data-zeyn-vlist data-zeyn-row-height="{height}" data-zeyn-block="{block}" data-zeyn-total="{total}" data-testid="{}.cards" data-text-error="{}">"#,
            r.scope,
            t("error"),
        ));
        if self.title.is_some() {
            out.push_str(&format!(
                r#"<div class="z-cards-head"><h2 class="z-cards-title" id="{id}-title">{title}</h2></div>"#
            ));
        }
        out.push_str(&format!(
            r#"<div class="z-vlist-view" data-zeyn-vlist-view tabindex="0" role="region" aria-label="{title}"><ul class="z-list" id="{id}-list" role="list" data-zeyn-rows>"#
        ));
        if let Some(p) = page {
            out.push_str(&self.cards(p, r, true));
        }
        out.push_str(&format!(
            r#"</ul></div><div class="z-cards-empty" data-zeyn-cards-empty{}>{}</div><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-cards-status></p></section>"#,
            if total == 0 { "" } else { " hidden" },
            self.empty_html(r, t("empty"))
        ));
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        if let Some(rows) = self.virtual_rows() {
            return self.render_virtual(out, r, page, rows);
        }
        let id = dom_id(r, "cards");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.cards.{key}")));
        let class = match (self.rail, self.list) {
            (true, _) if self.scrollbar => "z-rail z-rail-bar",
            (true, _) => "z-rail",
            (_, true) => "z-cards z-cards-rows",
            _ => "z-cards",
        };
        let labelled = if self.title.is_some() {
            format!(r#" aria-labelledby="{id}-title""#)
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<section class="{class}" data-zeyn-cards data-zeyn-paging="{}" data-testid="{}.cards"{labelled} data-text-loaded="{}" data-text-page="{}" data-text-page-only="{}" data-text-error="{}""#,
            if self.pages() { "pages" } else { "infinite" },
            r.scope,
            t("loaded"),
            t("page"),
            t("page_only"),
            t("error"),
        ));
        out.push_str(&format!(
            r#" style="--z-card-ratio:{:.3}""#,
            f64::from(self.image.1) / f64::from(self.image.0)
        ));
        if let Some(p) = page {
            out.push_str(&format!(
                r#" data-zeyn-offset="{}" data-zeyn-count="{}" data-zeyn-size="{}""#,
                p.offset,
                p.count,
                self.size()
            ));
            if let Some(total) = p.total {
                out.push_str(&format!(r#" data-zeyn-total="{total}""#));
            }
            if let Some(Cursor(next)) = &p.next {
                out.push_str(&format!(r#" data-zeyn-next="{}""#, escape(next)));
            }
        }
        out.push('>');
        let title = self
            .title
            .as_ref()
            .map(|t| {
                format!(
                    r#"<h2 class="z-cards-title" id="{id}-title">{}</h2>"#,
                    r.text(t)
                )
            })
            .unwrap_or_default();
        let sorts = if self.sorts.len() > 1 && !self.rail {
            let options: String = self
                .sorts
                .iter()
                .enumerate()
                .map(|(i, (label, s))| {
                    format!(
                        r#"<option value="{i}" data-sort="{}">{}</option>"#,
                        attr(s),
                        r.text(label)
                    )
                })
                .collect();
            format!(
                r#"<div class="z-cards-sort"><label for="{id}-sort">{}</label><div class="z-select" data-text-none=""><select id="{id}-sort" data-zeyn-cards-sort>{options}</select></div></div>"#,
                t("sort")
            )
        } else {
            String::new()
        };
        if self.rail {
            out.push_str(&format!(
                r#"<div class="z-rail-head">{title}<div class="z-rail-nav"><button type="button" class="z-rail-button" data-zeyn-rail-prev aria-controls="{id}-list" aria-label="{}" aria-disabled="true"><span aria-hidden="true">&#8249;</span></button><button type="button" class="z-rail-button" data-zeyn-rail-next aria-controls="{id}-list" aria-label="{}"><span aria-hidden="true">&#8250;</span></button></div></div>"#,
                t("previous"),
                t("next")
            ));
        } else if !title.is_empty() || !sorts.is_empty() {
            out.push_str(&format!(
                r#"<div class="z-cards-head">{title}{sorts}</div>"#
            ));
        }
        let list = match (self.rail, self.list) {
            (true, _) => "z-rail-list",
            (_, true) => "z-list",
            _ => "z-cards-list",
        };
        out.push_str(&format!(
            r#"<ul class="{list}" id="{id}-list" data-zeyn-rows>"#
        ));
        if let Some(p) = page {
            out.push_str(&self.cards(p, r, true));
        }
        out.push_str("</ul>");
        let empty = page.is_none_or(|p| p.count == 0);
        out.push_str(&format!(
            r#"<div class="z-cards-empty" data-zeyn-cards-empty{}>{}</div>"#,
            if empty { "" } else { " hidden" },
            self.empty_html(r, t("empty"))
        ));
        let more = page.is_some_and(|p| p.next.is_some());
        if self.pages() {
            out.push_str(&format!(
                r#"<nav class="z-cards-pager" aria-label="{}"><button type="button" class="z-button" data-zeyn-cards-prev aria-disabled="true">{}</button><span class="z-cards-page" data-zeyn-cards-page>{}</span><button type="button" class="z-button" data-zeyn-cards-next aria-disabled="{}">{}</button></nav>"#,
                t("pages"),
                t("previous_page"),
                page.map(|p| self.page_label(p, &t))
                    .unwrap_or_default(),
                !more,
                t("next_page")
            ));
        } else if !self.rail {
            out.push_str(&format!(
                r#"<div class="z-cards-more"><button type="button" class="z-button" data-zeyn-cards-more{}>{}</button></div>"#,
                if more { "" } else { " hidden" },
                t("load_more")
            ));
        }
        out.push_str(r#"<p class="z-sr-only" role="status" aria-live="polite" data-zeyn-cards-status></p></section>"#);
    }
}

impl<E: Model> GridSource for CardGrid<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self, _view: Option<&crate::grid::DataGridView>) -> DataQuery {
        let mut query = DataQuery::default();
        self.allow(&mut query);
        query
    }

    fn clean_view(&self, _view: crate::grid::DataGridView) -> crate::grid::DataGridView {
        crate::grid::DataGridView::default()
    }

    fn head_html(&self, _r: &Render, _view: Option<&crate::grid::DataGridView>) -> String {
        String::new()
    }

    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        self.allow(&mut query);
        query.time_zone = ctx.time_zone();
        let fetch = self.fetch;
        let offset = match &query.page {
            PageRequest::Offset { offset, .. } => *offset,
            PageRequest::After { .. } => 0,
        };
        Box::pin(async move {
            check_masks::<E>(&ctx)?;
            let page = fetch(ctx, query).await?;
            Ok(LoadedPage {
                count: page.items.len(),
                total: page.total,
                next: page.next,
                offset,
                totals: Vec::new(),
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        self.cards(page, r, false)
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        let sorts: Vec<&Sort> = self
            .sorts
            .iter()
            .map(|(_, s)| s)
            .collect();
        json!({
            "type": "object",
            "description": "Cards of rows. The server sets the page size; sort must be one of the offered orders.",
            "properties": {
                "page": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "enum": ["offset", "after"] },
                        "offset": { "type": "integer", "minimum": 0 },
                        "cursor": { "type": ["string", "null"] }
                    }
                },
                "sort": { "type": "array", "items": { "enum": sorts } }
            }
        })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for CardGrid<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        json!({
            "type": if self.rail { "rail" } else if self.list { "list" } else { "card_grid" },
            "model": E::NAME,
            "action": self.action,
            "title": self.title,
            "paging": self.paging,
            "sorts": self.sorts.iter().map(|(l, s)| json!({ "label": l, "field": s.field, "desc": s.desc })).collect::<Vec<_>>(),
            "search": self.search,
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        let scrolls = matches!(self.paging, Paging::Virtual { .. }) && self.list;
        if scrolls && self.builder.is_some() && self.row_height.is_none() {
            check.problem("a virtual list with `builder` needs `.row_height(px)`: virtual rows have one fixed height");
        }
        if scrolls && self.separator.is_some() {
            check.problem("`separator` does not work with virtual scrolling, where every row has the same height; leave it out or page the list");
        }
    }
}

impl<E: Model> From<CardGrid<E>> for Node {
    fn from(g: CardGrid<E>) -> Self {
        Node::new(g)
    }
}

/// Most suggestions one search box shows.
pub const MAX_SUGGESTIONS: u32 = 20;

type SuggestFn<E> = std::sync::Arc<dyn Fn(&E) -> Option<String> + Send + Sync>;
type SuggestItemFn<E> = std::sync::Arc<dyn Fn(&E) -> ListItem + Send + Sync>;

/// A search field with suggestions (W3C APG combobox with a list box popup). As the user types,
/// it waits `instant` milliseconds, then asks the server for suggestions: the action runs with the
/// text as [`DataQuery::q`] and the suggestion text of each row comes back. Enter, or picking a
/// suggestion, opens the results screen with the text in [`SearchParams`].
///
/// The server cuts the text to [`crate::MAX_SEARCH_QUERY`] characters, sets the number of
/// suggestions itself (8 unless set, at most [`MAX_SUGGESTIONS`]), checks the screen's access,
/// the feature's guards and the action's permission, and counts each call against the write rate
/// limit (`App::write_limit`), like every data source call.
///
/// Keyboard: Down and Up move through the suggestions (Down opens them), Enter searches with the
/// highlighted suggestion or the typed text, Escape closes the suggestions and then clears the
/// field.
///
/// `.item(|row| ListItem::new(title).image(src).subtitle(text).link(screen, params))` turns the
/// suggestions into rows with a small image, a title and a subtitle that go straight to the
/// record: picking one (click or Enter) opens its link through `nav.js`, middle click opens it
/// in a new tab, and Enter without a highlighted row still opens the results screen.
///
/// ```ignore
/// SearchBox::of(search_products).instant(150).results(search_results).suggestion(|p: &Product| p.name.clone())
/// ```
pub struct SearchBox<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    label: Text,
    delay: u32,
    results: Option<&'static str>,
    limit: u32,
    value: String,
    suggestion: SuggestFn<E>,
    item: Option<SuggestItemFn<E>>,
    drives: Vec<&'static str>,
}

impl<E: Model> SearchBox<E> {
    pub fn of<A>(_source: A) -> Self
    where
        A: crate::action::Action<Input = DataQuery, Output = Result<Page<E>>>,
    {
        Self {
            action: A::META,
            fetch: fetch_via::<A, E>,
            label: Text::key("zeyn.ui.search.label"),
            delay: 150,
            results: None,
            limit: 8,
            value: String::new(),
            suggestion: std::sync::Arc::new(first_text::<E>),
            item: None,
            drives: Vec::new(),
        }
    }

    /// Searches in place: Enter, picking a suggestion or clearing the field puts the text into
    /// the screen's `q` param and refreshes the nodes listening to `topics`, instead of opening a
    /// results screen. The screen's params need a `q: String` field (as [`SearchParams`]) for the
    /// listeners to read.
    pub fn drives(mut self, topics: impl super::Topics) -> Self {
        self.drives = super::topic_list(topics);
        self
    }

    /// How long to wait after the last key before asking for suggestions, in milliseconds
    /// (150 unless set, between 50 and 2000).
    pub fn instant(mut self, ms: u32) -> Self {
        self.delay = ms.clamp(50, 2000);
        self
    }

    /// The screen Enter opens with the text. Its params are [`SearchParams`].
    pub fn results<S: ScreenDef<Params = SearchParams>>(mut self, _screen: S) -> Self {
        const {
            assert!(
                S::META.route.is_some(),
                "the results screen of a search box must have a route"
            )
        };
        self.results = Some(S::META.name);
        self
    }

    /// The label of the field, read by screen readers. "Search" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }

    /// The text the field starts with, such as the search of a results screen.
    pub fn value(mut self, text: impl Into<String>) -> Self {
        self.value = text.into();
        self
    }

    /// How many suggestions to show, at most [`MAX_SUGGESTIONS`].
    pub fn limit(mut self, n: u32) -> Self {
        self.limit = n.clamp(1, MAX_SUGGESTIONS);
        self
    }

    /// Rich suggestions: each row as a [`ListItem`] with its title, subtitle, image (shown 40 by
    /// 56 pixels, following the `img-src` rules, with an empty alternative text because the
    /// title names the row) and link (its canonical address). Replaces [`SearchBox::suggestion`].
    pub fn item(mut self, item: impl Fn(&E) -> ListItem + Send + Sync + 'static) -> Self {
        self.item = Some(std::sync::Arc::new(item));
        self
    }

    /// The text a row suggests. The row's first unmasked text field unless set.
    pub fn suggestion(mut self, text: impl Fn(&E) -> String + Send + Sync + 'static) -> Self {
        self.suggestion = std::sync::Arc::new(move |row| Some(text(row)));
        self
    }
}

/// The first text field of a row that is neither the key nor masked.
fn first_text<E: Model>(row: &E) -> Option<String> {
    let i = E::fields().iter().position(|f| {
        matches!(f.kind, crate::value::FieldKind::Text) && f.mask.is_none() && !f.key
    })?;
    match row.value_at(i) {
        Some(Value::Text(t)) => Some(t),
        _ => None,
    }
}

impl<E: Model> DataSource for SearchBox<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let text: String = input
            .as_str()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(crate::MAX_SEARCH_QUERY)
            .collect();
        let query = DataQuery {
            page: PageRequest::After {
                cursor: None,
                limit: self.limit,
            },
            q: text.clone(),
            time_zone: ctx.time_zone(),
            ..DataQuery::default()
        };
        let fetch = self.fetch;
        let suggest = self.suggestion.clone();
        let item = self.item.clone();
        let view = ctx.clone();
        let rows = async move {
            if text.is_empty() {
                return Ok(Page::default());
            }
            check_masks::<E>(&ctx)?;
            fetch(ctx, query).await
        };
        Box::pin(async move {
            let page = rows.await?;
            if let Some(item) = item {
                let r = Render::new("s", view.locale(), view.i18n()).with_ctx(&view);
                return Ok(Json::Array(
                    page.items
                        .iter()
                        .map(|row| item(row).suggestion(&r))
                        .collect(),
                ));
            }
            let mut out: Vec<String> = Vec::new();
            for s in page
                .items
                .iter()
                .filter_map(|r| suggest(r))
            {
                if !s.is_empty() && !out.contains(&s) {
                    out.push(s);
                }
            }
            Ok(json!(out))
        })
    }
}

impl<E: Model> Component for SearchBox<E> {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "search");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.search.{key}")));
        let named = if self.drives.is_empty() {
            ""
        } else {
            r#" name="q""#
        };
        let results = match self.results {
            Some(s) => format!(r#" data-zeyn-results="{s}""#),
            None => String::new(),
        } + &super::topics_attr("data-zeyn-drives", &self.drives);
        out.push_str(&format!(
            r#"<form class="z-search" role="search" data-zeyn-searchbox data-zeyn-delay="{}" data-zeyn-max="{}"{results} data-text-count="{}" data-text-none="{}" data-testid="{}.search"><label class="z-sr-only" for="{id}">{}</label><span class="z-search-field"><span class="z-icon z-icon-search" aria-hidden="true"></span><input id="{id}"{named} type="search" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="{id}-list" autocomplete="off" spellcheck="false" enterkeyhint="search" maxlength="{}" placeholder="{}" value="{}"><ul class="z-search-list" role="listbox" id="{id}-list" aria-label="{}" hidden></ul></span><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-search-status></p></form>"#,
            self.delay,
            crate::MAX_SEARCH_QUERY,
            t("count"),
            t("none"),
            r.scope,
            r.text(&self.label),
            crate::MAX_SEARCH_QUERY,
            r.text(&self.label),
            escape(&self.value),
            t("suggestions"),
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "search_box", "model": E::NAME, "action": self.action, "results": self.results, "label": self.label, "drives": self.drives })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "suggest").then_some(self as &dyn DataSource)
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        if self.results.is_some() && !self.drives.is_empty() {
            check.problem("a search box either opens a results screen (`results`) or refreshes listeners in place (`drives`), not both");
        }
        for t in &self.drives {
            check.drives(t);
        }
    }
}

impl<E: Model> From<SearchBox<E>> for Node {
    fn from(s: SearchBox<E>) -> Self {
        Node::new(s)
    }
}
