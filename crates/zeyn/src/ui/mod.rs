//! Component library: layout containers, field editors and display components.
//!
//! Containers hold other nodes. Every node has a path: the index of a body item (`"1"`), then the
//! index of each child below it (`"1.0.2"`). The renderer writes the path in `data-zeyn-node` and
//! the endpoints accept it, so grids and forms inside containers keep working.

mod accordion;
mod alpha;
mod barcode;
mod calendar;
mod catalog;
mod chart;
mod chips;
mod code;
mod code_edit;
mod compare;
mod composer;
mod content;
mod controls;
mod countdown;
mod date;
mod diff;
mod dock;
mod document;
mod embed;
mod facets;
mod feed;
mod flow;
mod funnel;
mod gallery;
mod gantt;
mod heatmap;
mod hero;
mod inbox;
mod indicator;
pub(crate) mod inputs;
#[cfg(feature = "islands")]
mod island;
mod kanban;
mod layout;
mod lines;
mod list;
mod lookup;
mod mark;
#[cfg(feature = "markdown")]
mod markdown;
mod media;
mod menus;
mod org;
mod overlay;
mod pagination;
mod preview;
mod price;
mod quantity;
mod range;
mod rating;
mod reactions;
mod relative;
mod reorder;
mod reviews;
mod sanitize;
mod schema;
mod share;
mod shell;
mod stepper;
mod story;
mod table;
mod thread;
mod toc;
mod topics;
mod tour;
mod tree;
mod typing;
mod variants;
mod week;
mod widgets;
#[cfg(feature = "islands")]
pub(crate) use island::MARKER as ISLAND_MARKER;
#[cfg(feature = "islands")]
pub use island::{Canvas, Island, IslandLoad, Remote};
#[cfg(feature = "markdown")]
pub use markdown::{MAX_MARKDOWN, Markdown};
#[cfg(feature = "realtime")]
mod live;
#[cfg(feature = "realtime")]
pub use live::Live;
#[cfg(feature = "realtime")]
pub use live::{EventNode, OnEvent, OnEvents};

pub(crate) use dock::panel_ids as dock_panel_ids;
pub use dock::{DOCK_SIZE, Dock, DockArea, DockLayout, DockPanelState, DockSizes};
pub use embed::{
    Complete, EMBED_SANDBOX, Embed, EmbedPermission, ExternalLink, Fit, Given, Image, NoAlt,
    NoSize, NoTitle, PLACEHOLDER_BLUR_MAX, Placeholder,
};
pub use layout::{Card, Deferred, ListDetail, Panel, Row, Split, Stack, Tabs};
pub use shell::{MenuBar, MenuList, PropertyInspector, StatusBar, Toolbox};

pub use accordion::{Accordion, MAX_ACCORDION_SECTIONS};
pub use alpha::{AlphaIndex, OTHER_LETTER, alphabet};
pub use barcode::{
    Barcode, CODE128_QUIET, MAX_BARCODE_CHARS, MAX_QR_BYTES, QR_QUIET, QrCode, QrLevel, QrMatrix,
    code128_symbols, qr_error_correction, qr_format_bits, qr_version_bits,
};
pub use calendar::{Calendar, CalendarParams, CalendarSlot, CalendarView, MAX_CALENDAR_EVENTS};
pub use catalog::{CardGrid, ImageCard, MAX_SUGGESTIONS, Rail, SearchBox, SearchParams};
pub use chart::{Better, BulletCell, Chart, ChartKind, Kpi, SparklineCell};
pub use chips::{Chips, MAX_CHIPS};
pub use code::{CodeLang, CodeView, JsonView, MAX_CODE_BYTES, MAX_CODE_LINES, highlight};
pub(crate) use code_edit::{CheckSource, CompleteSource, EditorHtml, render_editor};
pub use code_edit::{
    CodeEditor, CodeError, CodeQuery, Completion, CompletionKind, MAX_CODE_ERRORS,
    MAX_CODE_MESSAGE_CHARS, MAX_CODE_QUERY_BYTES, MAX_COMPLETION_CHARS, MAX_COMPLETIONS,
};
pub use compare::{
    CompareCell, ComparisonTable, ImageCompare, MAX_COMPARE_PLANS, MAX_COMPARE_ROWS,
};
pub use composer::{
    ChatSend, Composer, DEFAULT_MESSAGE_CHARS, MAX_MENTION_QUERY, MAX_MENTIONS,
    MAX_MESSAGE_MENTIONS, SendKey,
};
pub use content::{
    AutoGrid, Avatar, AvatarGroup, AvatarSize, Divider, Heading, Kbd, MAX_AVATAR_GROUP,
    MAX_KBD_KEYS, MetaLine, Paragraph, PrevNext, Quote, Spoiler,
};
pub use controls::{Control, LengthUnit, MAX_PALETTE, is_hex_color};
pub(crate) use controls::{
    Slot, choices_text, read_choices, render_bool, render_choice, render_choices, render_code,
    render_number, render_text,
};
pub use countdown::Countdown;
pub use date::DateOptions;
pub(crate) use date::{input_text, picker_config, range_reversed, render_date_input, render_range};
pub use diff::{DiffLayout, DiffView, MAX_DIFF_FIELDS};
pub use document::{
    DocPage, DocSearch, DocumentViewer, MAX_DOCUMENT_PAGES, MAX_DOCUMENT_QUERY, MAX_PAGE_BYTES,
    MAX_PAGE_HITS, OutlineItem, PageHit, PageSize,
};
pub use facets::{Facet, FacetPanel, MAX_FACET_OPTIONS, MAX_FACETS};
pub use feed::{Feed, MAX_FEED_BATCH, MAX_POST_MEDIA, Post};
pub use flow::{FlowDiagram, MAX_FLOW_EDGES, MAX_FLOW_NODES};
pub use funnel::{ActivityRings, Funnel, MAX_FUNNEL_STAGES, MAX_RINGS};
pub use gallery::{MAX_GALLERY_IMAGES, ProductGallery};
pub use gantt::{Gantt, MAX_GANTT_DAYS, MAX_GANTT_TASKS};
pub use heatmap::{HEATMAP_LEVELS, Heatmap, MAX_HEATMAP_WEEKS, heat_level};
pub use hero::{HERO_SECONDS, Hero, MAX_HERO_SLIDES};
pub use inbox::{Inbox, InboxSelection, MAX_INBOX, MAX_UNREAD, NotificationCenter, UnreadCount};
pub use indicator::{Indicator, MAX_INDICATOR_COUNT};
pub use inputs::{mask_example, mask_fits, mask_raw};
pub use kanban::{KANBAN_BLOCK, Kanban};
pub(crate) use lines::LinesEditor;
pub use lines::{Lines, MAX_LINES};
pub use list::{List, ListItem, MAX_REPEAT, Repeat};
pub use lookup::{
    LookupItem, LookupQuery, LookupValue, MAX_LOOKUP_COLUMNS, MAX_LOOKUP_IDS, MAX_LOOKUP_PAGE,
};
pub(crate) use lookup::{parse_ids, render_lookup};
pub use mark::{Highlight, MAX_HIGHLIGHT_TERM_CHARS, MAX_HIGHLIGHT_TERMS, highlight_ranges};
pub use media::{Audio, PLAYBACK_SPEEDS, Video};
pub use menus::MenuButton;
pub use org::{MAX_ORG_DEPTH, MAX_ORG_NODES, OrgChart, OrgNode};
pub(crate) use overlay::tip;
pub use overlay::{BUSY_DELAY_MS, Busy, MAX_TIP, Popover};
pub use pagination::{MAX_PAGES_AROUND, Pagination, page_window};
pub use price::{Price, PriceUnit};
pub use quantity::{MAX_QUANTITY, QuantityChange, QuantityState, QuantityStepper};
pub use range::RangeSlider;
pub(crate) use rating::halves as rating_halves;
pub use rating::{DEFAULT_RATING, MAX_RATING};
pub use reactions::{MAX_REACTIONS, ReactionPress, ReactionState, Reactions};
pub use relative::RelativeTime;
pub use reviews::ReviewSummary;
pub use sanitize::{MAX_USE_ELEMENTS, sanitize_markup};
pub use schema::{
    FieldSpec, FormSchema, MAX_FIELD_NAME, MAX_SCHEMA_FIELDS, MAX_SCHEMA_OPTIONS, MAX_SCHEMA_TEXT,
    SchemaForm, SpecKind,
};
pub(crate) use schema::{limits_attr, open_section, render_spec, render_templates};
pub use share::{CopyButton, MAX_COPY_BYTES, Share};
pub use stepper::{MAX_STEPS, Stepper};
pub use story::{MAX_STORY_RINGS, StoryRing, StoryRow, StoryState};
pub use table::{MAX_TABLE_COLUMNS, MAX_TABLE_ROWS, Table};
pub use thread::{
    Delivery, MAX_CONVERSATION, MAX_MESSAGE_CHARS, MAX_REPLY_CHARS, MAX_SHOWN_FILES, MessageThread,
    REPLY_BUFFER, REPLY_SECONDS, ReplyRequest, THREAD_PAGE, ThreadQuery,
};
#[cfg(feature = "server")]
pub use thread::{Reply, ReplySender, StreamSource};
pub use toc::{MAX_TOC_ITEMS, Toc};
pub use topics::{Listen, Listens, MAX_TOPICS, PageCheck, Topics, valid_topic};
pub(crate) use topics::{topic_list, topics_attr};
pub use tour::{MAX_TOUR_STOPS, Tour, TourStop};
pub use tree::{MAX_TREE_CHILDREN, MAX_TREE_DEPTH, Tree, TreeItem, TreeMove, TreePlace, TreeQuery};
pub use typing::{MAX_TYPING_PEOPLE, TYPING_EVERY_SECONDS, TYPING_SHOWN_SECONDS, TypingIndicator};
pub use variants::{MAX_VARIANT_GROUPS, MAX_VARIANT_OPTIONS, VariantGroup, VariantPicker};
pub use week::{MAX_WEEK_ROWS, WeekDay, WeekSchedule};
pub use widgets::{
    ActionButton, Alert, Badge, Breadcrumb, ClearPrefs, DescriptionList, DescriptionOf, EmptyState,
    LinkButton, Meter, MeterState, Progress, ProgressRing, RING_SIZES, Skeleton, ThemePicker,
    Timeline,
};

use crate::action::{Action, ActionMeta, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::render::Render;
use crate::screen::{Node, Screen};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as Json;
use std::fmt;
use std::marker::PhantomData;

/// Most levels a node path may have. Deeper screens are not useful and paths come from clients.
pub const MAX_NODE_DEPTH: usize = 16;

/// Address of a node in a screen: `"0"` for the first body item, `"1.0.2"` for a nested one.
///
/// Read from clients as a number (`0`, kept for older clients) or a dotted string.
///
/// ```
/// use zeyn::ui::NodePath;
/// let p: NodePath = serde_json::from_str("\"1.0.2\"").unwrap();
/// assert_eq!(p.as_str(), "1.0.2");
/// let old: NodePath = serde_json::from_str("3").unwrap();
/// assert_eq!(old.as_str(), "3");
/// assert!(serde_json::from_str::<NodePath>("\"1..2\"").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodePath(String);

impl NodePath {
    /// A checked path, or `None` when it is not dotted indexes.
    pub fn parse(s: &str) -> Option<NodePath> {
        let parts: Vec<&str> = s.split('.').collect();
        let valid = parts.len() <= MAX_NODE_DEPTH
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()));
        valid.then(|| {
            NodePath(
                parts
                    .iter()
                    .map(|p| {
                        p.parse::<usize>()
                            .unwrap_or(0)
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("."),
            )
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Indexes from the screen body down.
    pub fn indexes(&self) -> impl Iterator<Item = usize> + '_ {
        self.0
            .split('.')
            .filter_map(|p| p.parse().ok())
    }
}

impl From<usize> for NodePath {
    fn from(i: usize) -> Self {
        NodePath(i.to_string())
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for NodePath {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NodePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Index(u64),
            Path(String),
        }
        let text = match Raw::deserialize(deserializer)? {
            Raw::Index(i) => i.to_string(),
            Raw::Path(s) => s,
        };
        NodePath::parse(&text).ok_or_else(|| {
            serde::de::Error::custom("node must be an index or a dotted path such as 1.0.2")
        })
    }
}

impl Screen {
    /// The node at a path, following container children.
    pub fn node(&self, path: &NodePath) -> Option<&Node> {
        let mut indexes = path.indexes();
        let mut node = self.nodes().get(indexes.next()?)?;
        for i in indexes {
            node = node.children().get(i)?;
        }
        Some(node)
    }

    /// Every node rendered with the page, depth first, with its path. Children of closed lazy
    /// containers are left out; they are loaded with their panel.
    pub fn walk(&self) -> Vec<(NodePath, &Node)> {
        let mut out = Vec::new();
        for (i, node) in self.nodes().iter().enumerate() {
            walk_into(NodePath::from(i), node, &mut out);
        }
        out
    }

    /// The node at `path` and every node below it that renders with it.
    pub fn walk_from(&self, path: &NodePath) -> Vec<(NodePath, &Node)> {
        let mut out = Vec::new();
        if let Some(node) = self.node(path) {
            walk_into(path.clone(), node, &mut out);
        }
        out
    }
}

fn walk_into<'a>(path: NodePath, node: &'a Node, out: &mut Vec<(NodePath, &'a Node)>) {
    let children = node.eager_children();
    out.push((path.clone(), node));
    for (i, child) in children.iter().enumerate() {
        walk_into(NodePath(format!("{}.{i}", path.0)), child, out);
    }
}

/// Writes a node with its wrapper and path. Grids use their preloaded first page when there is one.
pub fn render_node(out: &mut String, r: &Render, path: &str, node: &Node) {
    out.push_str(&format!(r#"<div class="z-node" data-zeyn-node="{path}">"#));
    let r = Render { path, ..*r };
    #[cfg(debug_assertions)]
    let start = out.len();
    match (node.grid(), r.loaded.and_then(|l| l.get(path))) {
        (Some(grid), Some(page)) => grid.render_loaded(out, &r, page),
        _ => node.render(out, &r),
    }
    #[cfg(debug_assertions)]
    node.tag(out, start);
    out.push_str("</div>");
}

/// Writes child `index` of the container being rendered. Containers call this for each child.
pub fn render_child(out: &mut String, r: &Render, index: usize, node: &Node) {
    let path = if r.path.is_empty() {
        index.to_string()
    } else {
        format!("{}.{index}", r.path)
    };
    render_node(out, r, &path, node);
}

/// A DOM id unique on the page for the node being rendered: `{scope}-{what}-{path}`.
pub(crate) fn dom_id(r: &Render, what: &str) -> String {
    format!("{}-{what}-{}", r.scope, r.path.replace('.', "-"))
}

/// Data a component fetches from the server through an action, such as lookup options or tree
/// children. The server finds it by node path and key, checks the screen's access, the feature's
/// guards and the action's permission, then runs it.
pub trait DataSource: Send + Sync {
    fn action(&self) -> ActionMeta;

    /// Runs the action with the client's input. The input is untrusted: it is read as the
    /// action's input type, and anything that does not fit is refused.
    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>>;

    /// The group and the list id of a source joined to other lists, such as a sortable list with
    /// `connect(group, id)`. Rows move only between sources of the same group.
    fn link(&self) -> Option<(&str, &str)> {
        None
    }

    /// Runs the action for rows that come from the connected list with id `from`. The server
    /// calls it only after it found that list on the same screen, in the same group, and
    /// checked the permission of its action.
    fn fetch_from(&self, _ctx: Ctx, _input: Json, _from: String) -> BoxFuture<Result<Json>> {
        Box::pin(async { Err(crate::action::Error::NotFound) })
    }
}

/// A data source backed by an action that returns `Result<T>`.
pub(crate) struct ActionSource<A>(PhantomData<fn() -> A>);

impl<A> ActionSource<A> {
    pub(crate) fn new() -> Self {
        ActionSource(PhantomData)
    }
}

impl<A, T> DataSource for ActionSource<A>
where
    A: Action<Output = Result<T>>,
    A::Input: DeserializeOwned,
    T: Serialize + Send + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let input: A::Input =
                serde_json::from_value(input).map_err(|_| crate::action::Error::NotFound)?;
            let output = A::call(ctx.clone(), input).await??;
            crate::privacy::client_json(&output, &ctx)
                .map_err(|e| crate::action::Error::Internal(e.to_string()))
        })
    }
}
