//! The Zeyn component catalog, written with Zeyn.
//!
//! Every component has one file under `src/examples/`. Its functions build the live examples, and
//! the same file is embedded with `include_str!` and shown as their code, so the code on a page is
//! always the code that runs. The API tables come from the framework's source through `build.rs`.

pub mod data;
pub mod examples;
pub mod page;
pub mod site;

use zeyn::prelude::*;
use zeyn::ui::SearchParams;

include!(concat!(env!("OUT_DIR"), "/api.rs"));

/// The params of every component page. Examples whose filters drive other nodes keep their
/// values here, so the listeners read them from the screen params; the other examples ignore
/// them.
#[params]
#[derive(Default)]
pub struct Demo {
    pub kind: Option<data::Kind>,
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub q: Option<String>,
}

/// One live example: the function of the component's file that builds it.
pub struct Example {
    pub func: &'static str,
    pub build: fn(&Ctx, &Demo) -> Node,
}

/// An [`Example`] of the function `f(ctx)` of the current file.
#[macro_export]
macro_rules! ex {
    ($f:ident) => {
        $crate::Example {
            func: stringify!($f),
            build: |ctx, _| $f(ctx),
        }
    };
}

/// An [`Example`] of the function `f(ctx, demo)` of the current file, which reads the page's
/// params.
#[macro_export]
macro_rules! exd {
    ($f:ident) => {
        $crate::Example {
            func: stringify!($f),
            build: $f,
        }
    };
}

/// What a component's file says about it.
pub struct Entry {
    /// The name on the index and the page title, usually the Rust name, such as `DataGrid`.
    pub name: &'static str,
    /// Path and anchor of its page on the documentation site, such as `components/content#avatar`.
    pub docs: &'static str,
    /// Framework types whose methods the API tab lists.
    pub api: &'static [&'static str],
    /// Examples of the Overview tab. The first one is also the preview on the index.
    pub overview: &'static [Example],
    /// Variants on the Examples tab.
    pub examples: &'static [Example],
}

/// A component of the catalog: its file, its place and its route.
pub struct Component {
    pub slug: &'static str,
    pub category: &'static str,
    pub route: &'static str,
    pub entry: &'static Entry,
    pub source: &'static str,
}

impl Component {
    /// The one line description, from the locale files.
    pub fn summary(&self) -> Text {
        Text::key(format!("cx.summary.{}", self.slug))
    }
}

/// The params type of a catalog page: `()` unless the entry names one.
macro_rules! page_params {
    () => {
        ()
    };
    ($p:ty) => {
        $p
    };
}

impl From<()> for Demo {
    fn from(_: ()) -> Demo {
        Demo::default()
    }
}

macro_rules! catalog {
    ($( $cat:literal $icon:ident { $( $slug:ident $route:literal $( ($p:ty) )? ; )* } )*) => {
        $( $(
            #[doc = concat!("The page of `", stringify!($slug), "`.")]
            #[screen(route = $route, public)]
            pub fn $slug(demo: page_params!($($p)?), ctx: Ctx) -> Screen {
                page::component_screen(stringify!($slug), &ctx, &Demo::from(demo))
            }
        )* )*

        /// Every component, by category in menu order.
        pub static COMPONENTS: &[Component] = &[ $( $( Component {
            slug: stringify!($slug),
            category: $cat,
            route: $route,
            entry: &examples::$slug::ENTRY,
            source: include_str!(concat!("examples/", stringify!($slug), ".rs")),
        }, )* )* ];

        /// The categories in menu order.
        pub const CATEGORIES: &[&str] = &[ $( $cat, )* ];

        fn with_screens(feature: Feature) -> Feature {
            feature $( $( .screen($slug) )* )*
                .menu(|m| m
                    .group(t!("cx.start"), Icon::Home)
                    .item(t!("cx.all"), catalog_home)
                    $( .group(Text::key(concat!("cx.cat.", $cat)), Icon::$icon) $( .item(examples::$slug::ENTRY.name, $slug) )* )*
                )
        }
    };
}

catalog! {
    "layout" Table {
        stack "/ui/stack";
        row "/ui/row";
        card "/ui/card";
        panel "/ui/panel";
        tabs "/ui/tabs";
        split "/ui/split";
        auto_grid "/ui/auto-grid";
        screen_header "/ui/screen-header";
    }
    "text" Edit {
        heading "/ui/heading";
        paragraph "/ui/paragraph";
        meta_line "/ui/meta-line";
        badge "/ui/badge";
        avatar "/ui/avatar";
        icon "/ui/icon";
        description_list "/ui/description-list";
        timeline "/ui/timeline";
        markdown "/ui/markdown";
        spoiler "/ui/spoiler";
        relative_time "/ui/relative-time";
    }
    "navigation" Home {
        breadcrumb "/ui/breadcrumb";
        link_button "/ui/link-button";
        prev_next "/ui/prev-next";
        alpha_index "/ui/alpha-index";
        chips "/ui/chips";
        menu_button "/ui/menu-button";
        menu_bar "/ui/menu-bar";
        external_link "/ui/external-link";
        share "/ui/share";
    }
    "forms" Edit {
        form "/ui/form";
        controls "/ui/controls";
        date_pickers "/ui/date-pickers";
        lookup "/ui/lookup";
        line_editor "/ui/line-editor";
        masked_inputs "/ui/masked-inputs";
        rating "/ui/rating";
        action_button "/ui/action-button";
    }
    "data" Table {
        data_grid "/ui/data-grid";
        card_grid "/ui/card-grid";
        image_card "/ui/image-card";
        rail "/ui/rail";
        list "/ui/list";
        search_box "/ui/search-box";
        topics "/ui/topics" (Demo);
        tree "/ui/tree";
        week_schedule "/ui/week-schedule";
    }
    "media" Box {
        image "/ui/image";
        embed "/ui/embed";
        hero "/ui/hero";
    }
    "feedback" Plus {
        alert "/ui/alert";
        progress "/ui/progress";
        skeleton "/ui/skeleton";
        empty_state "/ui/empty-state";
        popover "/ui/popover";
        busy "/ui/busy";
        toast "/ui/toast";
        clear_prefs "/ui/clear-prefs";
        reactions "/ui/reactions";
    }
    "charts" Chart {
        chart "/ui/chart";
        kpi "/ui/kpi";
    }
    "shell" Box {
        toolbox "/ui/toolbox";
        status_bar "/ui/status-bar";
        app_header "/ui/app-header";
        theme_picker "/ui/theme-picker";
    }
}

/// The component with this slug.
pub fn component(slug: &str) -> Option<&'static Component> {
    COMPONENTS
        .iter()
        .find(|c| c.slug == slug)
}

/// A component as the search finds it: its name and its description in the user's language.
#[model]
pub struct Found {
    pub name: String,
    pub summary: String,
}

/// Components whose name or description has the text, in catalog order.
pub fn matching(q: &str, ctx: &Ctx) -> Vec<&'static Component> {
    let rows: Vec<Found> = COMPONENTS
        .iter()
        .map(|c| Found {
            name: c.entry.name.to_string(),
            summary: ctx
                .i18n()
                .text(ctx.locale(), &c.summary()),
        })
        .collect();
    let query = DataQuery {
        q: q.to_string(),
        page: zeyn::PageRequest::Offset {
            offset: 0,
            limit: zeyn::MAX_PAGE_SIZE,
        },
        ..DataQuery::default()
    };
    let names: Vec<String> = query
        .apply(rows)
        .items
        .into_iter()
        .map(|f| f.name)
        .collect();
    COMPONENTS
        .iter()
        .filter(|c| names.iter().any(|n| n == c.entry.name))
        .collect()
}

/// Search suggestions: the components whose name or description has the text.
#[action(public)]
pub async fn find_components(q: DataQuery, ctx: Ctx) -> Result<Page<Found>> {
    let rows: Vec<Found> = COMPONENTS
        .iter()
        .map(|c| Found {
            name: c.entry.name.to_string(),
            summary: ctx
                .i18n()
                .text(ctx.locale(), &c.summary()),
        })
        .collect();
    Ok(q.apply(rows))
}

/// The index: every component by category with a live preview, or the ones the search found.
#[screen(route = "/", public)]
pub fn catalog_home(p: SearchParams, ctx: Ctx) -> Screen {
    page::index_screen(&p.q, &ctx)
}

/// The catalog feature: the index, a page per component and the search.
pub fn feature() -> Feature {
    with_screens(
        Feature::new("components")
            .screen(catalog_home)
            .screen(examples::image_card::product)
            .screen(examples::screen_header::orders_page),
    )
}

/// The catalog app.
pub fn app() -> std::result::Result<App, String> {
    let i18n = zeyn::I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
    Ok(App::new(t!("cx.app"))
        .i18n(i18n)
        .csp(|c| c.frame_src(["https://www.openstreetmap.org"]))
        .home(catalog_home)
        .header(examples::app_header::catalog_header)
        .prefetch(zeyn::Prefetch::Moderate)
        .client_prefs(|p| {
            p.key(examples::tabs::DELIVERY_TAB)
                .key(examples::embed::MAP_NOTICE)
        })
        .feature(feature()))
}
