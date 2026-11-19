//! [`Pagination`]: links to the pages of a list that a screen shows page by page.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(Pagination);

/// Most pages [`Pagination::around`] shows on each side of the current page.
pub const MAX_PAGES_AROUND: u32 = 4;

/// The pages a pagination shows for `current` of `total`, in order, with `None` where pages are
/// left out. The first and the last page always show, with `around` pages on each side of the
/// current one. A gap of one page shows that page instead, so a gap always hides two or more.
/// Pages count from 1; a `current` outside `1..=total` is brought inside.
///
/// ```
/// use zeyn::ui::page_window;
/// assert_eq!(page_window(1, 3, 1), [Some(1), Some(2), Some(3)]);
/// assert_eq!(
///     page_window(6, 20, 1),
///     [Some(1), None, Some(5), Some(6), Some(7), None, Some(20)]
/// );
/// assert_eq!(page_window(3, 20, 1), [Some(1), Some(2), Some(3), Some(4), None, Some(20)]);
/// assert!(page_window(1, 0, 1).is_empty());
/// ```
pub fn page_window(current: u32, total: u32, around: u32) -> Vec<Option<u32>> {
    if total == 0 {
        return Vec::new();
    }
    let current = current.clamp(1, total);
    let low = current.saturating_sub(around).max(1);
    let high = current
        .saturating_add(around)
        .min(total);
    let mut pages = Vec::new();
    let mut last = 0;
    for page in std::iter::once(1)
        .chain(low..=high)
        .chain(std::iter::once(total))
    {
        if page <= last {
            continue;
        }
        match page - last {
            1 => {}
            2 => pages.push(Some(last + 1)),
            _ => pages.push(None),
        }
        pages.push(Some(page));
        last = page;
    }
    pages
}

type ParamsFn = Box<dyn Fn(u32) -> Json + Send + Sync>;

/// Links to the pages of a list: previous, the page numbers and next. Each one is a link to the
/// same screen with the page in its params, so the server renders the page it is asked for and
/// nothing is kept in the browser; the links open through `nav.js` without a page reload and
/// work without script. `DataGrid`, `List` and `CardGrid` page themselves; this is for a screen
/// that pages its own content, such as the articles of a blog.
///
/// The current page is marked with `aria-current="page"`, every number is named "Page 3" for
/// screen readers, and previous and next carry `rel="prev"` and `rel="next"`. Pages count from 1.
/// The page number comes from the address, so the screen brings it into range itself; the
/// component clamps what it marks as current. With one page or none it renders nothing.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Pagination;
///
/// #[params]
/// pub struct BlogParams {
///     pub page: u32,
/// }
///
/// #[screen(route = "/blog", public)]
/// pub fn blog(p: BlogParams) -> Screen {
///     Screen::new("Blog").body(
///         Pagination::new(blog, |page| BlogParams { page })
///             .pages(12)
///             .current(p.page),
///     )
/// }
/// ```
pub struct Pagination {
    screen: &'static str,
    params: ParamsFn,
    pages: u32,
    current: u32,
    around: u32,
}

impl Pagination {
    /// Links that open `screen` with the params `params(page)` returns. Params of the wrong type
    /// do not compile.
    pub fn new<S: ScreenDef>(
        _screen: S,
        params: impl Fn(u32) -> S::Params + Send + Sync + 'static,
    ) -> Self {
        Pagination {
            screen: S::META.name,
            params: Box::new(move |page| serde_json::to_value(params(page)).unwrap_or(Json::Null)),
            pages: 0,
            current: 1,
            around: 1,
        }
    }

    /// How many pages there are.
    pub fn pages(mut self, pages: u32) -> Self {
        self.pages = pages;
        self
    }

    /// The page being shown, counted from 1.
    pub fn current(mut self, page: u32) -> Self {
        self.current = page;
        self
    }

    /// How many page numbers show on each side of the current one: 1 unless set, at most
    /// [`MAX_PAGES_AROUND`].
    pub fn around(mut self, pages: u32) -> Self {
        self.around = pages.min(MAX_PAGES_AROUND);
        self
    }

    fn href(&self, r: &Render, page: u32) -> String {
        screen_href(r, self.screen, &(self.params)(page))
    }

    /// The previous or next link, or a disabled stand in at the ends.
    fn step(&self, out: &mut String, r: &Render, page: Option<u32>, rel: &str, label: &str) {
        match page {
            Some(page) => out.push_str(&format!(
                r#"<li><a class="z-pagelink z-pagelink-step" href="{}" data-zeyn-base-href rel="{rel}">{label}</a></li>"#,
                self.href(r, page)
            )),
            None => out.push_str(&format!(
                r#"<li><span class="z-pagelink z-pagelink-step" aria-disabled="true">{label}</span></li>"#
            )),
        }
    }
}

impl Component for Pagination {
    fn render(&self, out: &mut String, r: &Render) {
        if self.pages <= 1 {
            return;
        }
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.pagination.{key}")));
        let current = self.current.clamp(1, self.pages);
        out.push_str(&format!(
            r#"<nav class="z-pagination" aria-label="{}"><ul class="z-pagination-list">"#,
            t("label")
        ));
        self.step(
            out,
            r,
            (current > 1).then(|| current - 1),
            "prev",
            &t("previous"),
        );
        for page in page_window(current, self.pages, self.around) {
            match page {
                Some(page) => {
                    let named = Text::key("zeyn.ui.pagination.page").arg("n", page.to_string());
                    out.push_str(&format!(
                        r#"<li><a class="z-pagelink" href="{}" data-zeyn-base-href aria-label="{}"{}>{page}</a></li>"#,
                        self.href(r, page),
                        r.text(&named),
                        if page == current {
                            r#" aria-current="page""#
                        } else {
                            ""
                        }
                    ));
                }
                None => {
                    out.push_str(r#"<li class="z-pagination-gap" aria-hidden="true">&#8230;</li>"#)
                }
            }
        }
        self.step(
            out,
            r,
            (current < self.pages).then(|| current + 1),
            "next",
            &t("next"),
        );
        out.push_str("</ul></nav>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "pagination",
            "screen": self.screen,
            "pages": self.pages,
            "current": self.current.clamp(1, self.pages.max(1)),
            "around": self.around,
        })
    }
}
