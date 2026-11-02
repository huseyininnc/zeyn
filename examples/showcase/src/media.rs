//! Media library: a catalog of short films and songs with a player, served with range requests.
//!
//! Films keep the play position on the server per user, so it follows the user to any device.
//! Songs keep no positions on the server, so the player keeps them in the browser instead.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use zeyn::prelude::*;
use zeyn::render::{attr, escape};
use zeyn::{BoxFuture, Component, Media, MediaSource, Render};

/// Permission to watch and listen.
const PLAY: &str = "showcase.media.play";

/// One item of the catalog.
struct Item {
    key: &'static str,
    video: bool,
    title: &'static str,
    subtitle: &'static str,
}

const CATALOG: [Item; 5] = [
    Item { key: "harbor", video: true, title: "media.harbor", subtitle: "media.harbor_sub" },
    Item { key: "zoom", video: true, title: "media.zoom", subtitle: "media.zoom_sub" },
    Item { key: "dawn", video: false, title: "media.dawn", subtitle: "media.artist" },
    Item { key: "tide", video: false, title: "media.tide", subtitle: "media.artist" },
    Item { key: "lanterns", video: false, title: "media.lanterns", subtitle: "media.artist" },
];

/// Files of the sample folder. Ids are looked up in a fixed list; a client never names a path.
struct Folder {
    files: &'static [&'static str],
    positions: Option<Arc<Mutex<HashMap<(String, String), f64>>>>,
}

impl Folder {
    fn path(&self, id: &str) -> Option<PathBuf> {
        self.files.contains(&id).then(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/media")).join(id))
    }

    fn user_key(ctx: &Ctx, id: String) -> (String, String) {
        (ctx.identity().user.clone().unwrap_or_default(), id)
    }
}

impl MediaSource for Folder {
    fn open(&self, _ctx: Ctx, id: String) -> BoxFuture<Result<Media>> {
        let path = self.path(&id);
        Box::pin(async move {
            let path = path.ok_or(Error::NotFound)?;
            Media::file(path).map_err(|_| Error::NotFound)
        })
    }

    fn position(&self, ctx: Ctx, id: String) -> BoxFuture<Result<Option<f64>>> {
        let at = self
            .positions
            .as_ref()
            .map(|p| p.lock().map(|m| m.get(&Self::user_key(&ctx, id)).copied()).unwrap_or(None));
        Box::pin(async move { at.ok_or(Error::NotFound) })
    }

    fn save_position(&self, ctx: Ctx, id: String, seconds: f64) -> BoxFuture<Result<()>> {
        let saved = self.positions.as_ref().map(|p| {
            if let Ok(mut m) = p.lock() {
                m.insert(Self::user_key(&ctx, id), seconds);
            }
        });
        Box::pin(async move { saved.ok_or(Error::NotFound) })
    }
}

/// Which item the player shows.
#[params]
pub struct PlayParams {
    pub item: String,
}

/// A shelf of catalog tiles; each opens the item in the player.
struct Shelf {
    title: Text,
    items: Vec<(&'static str, Text, Text, bool)>,
    current: &'static str,
}

impl Component for Shelf {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<section class="z-shelf" aria-label="{0}"><h2 class="z-shelf-title">{0}</h2><ul class="z-shelf-list">"#,
            r.text(&self.title)
        ));
        for (key, title, subtitle, video) in &self.items {
            let effect = Effect::Navigate { screen: "media_play", params: serde_json::json!({ "item": key }) };
            let current = if *key == self.current { r#" aria-current="true""# } else { "" };
            let kind = if *video { "video" } else { "audio" };
            out.push_str(&format!(
                r#"<li><button type="button" class="z-shelf-tile z-shelf-{kind}" data-zeyn-effect="{}"{current} data-testid="{}.shelf.{}"><span class="z-shelf-name">{}</span><span class="z-shelf-sub">{}</span></button></li>"#,
                attr(&effect.to_json(r)),
                r.scope,
                escape(key),
                r.text(title),
                r.text(subtitle)
            ));
        }
        out.push_str("</ul></section>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "shelf", "title": self.title, "items": self.items.iter().map(|i| i.0).collect::<Vec<_>>() })
    }
}

impl From<Shelf> for Node {
    fn from(s: Shelf) -> Self {
        Node::new(s)
    }
}

fn shelf(title: &'static str, video: bool, current: &'static str) -> Shelf {
    Shelf {
        title: Text::key(title),
        items: CATALOG
            .iter()
            .filter(|i| i.video == video)
            .map(|i| (i.key, Text::key(i.title), Text::key(i.subtitle), i.video))
            .collect(),
        current,
    }
}

fn player(item: &Item) -> Node {
    match item.key {
        "harbor" => Video::new("films", Text::key(item.title))
            .subtitle(Text::key(item.subtitle))
            .file("harbor.webm")
            .file("harbor.mp4")
            .hls("harbor.m3u8")
            .poster("harbor.jpg")
            .captions("harbor.en.vtt", "en", "English")
            .captions("harbor.tr.vtt", "tr", "Türkçe")
            .into(),
        "zoom" => Video::new("films", Text::key(item.title))
            .subtitle(Text::key(item.subtitle))
            .file("zoom.mp4")
            .poster("zoom.jpg")
            .into(),
        key => {
            let audio = Audio::new("music", Text::key(item.title))
                .subtitle(Text::key(item.subtitle))
                .file(format!("{key}.mp3"));
            if key == "dawn" { audio.captions("dawn.en.vtt", "en", "English").into() } else { audio.into() }
        }
    }
}

fn library_of(key: &str) -> Result<Screen> {
    let item = CATALOG.iter().find(|i| i.key == key).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("media.title")).body(
        Row::new().col(player(item)).weight(2).col(Stack::new().add(shelf("media.films", true, item.key)).add(shelf(
            "media.songs",
            false,
            item.key,
        ))),
    ))
}

#[screen(route = "/library", permission = "showcase.media.play")]
pub fn media_library() -> Result<Screen> {
    library_of("harbor")
}

#[screen(route = "/play", permission = "showcase.media.play")]
pub fn media_play(p: PlayParams) -> Result<Screen> {
    library_of(&p.item)
}

/// Content from other sites: a click to load player on a host of the app's `frame-src`, and an
/// embed whose host is not listed, which renders a warning instead of a frame.
#[screen(route = "/external", permission = "showcase.media.play")]
pub fn media_external() -> Result<Screen> {
    Ok(Screen::new(t!("media.external")).body(
        Row::new()
            .col(
                Panel::new(t!("media.embed_card")).add(
                    Embed::url("https://www.youtube-nocookie.com/embed/aqz-KE-bpKQ")
                        .title(t!("media.embed_title"))
                        .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
                        .click_to_load("/_zeyn/media/films/harbor.jpg"),
                ),
            )
            .col(
                Panel::new(t!("media.blocked_card"))
                    .add(Embed::url("https://maps.example.net/embed?q=harbor").title(t!("media.blocked_title"))),
            ),
    ))
}

pub fn feature() -> Feature {
    Feature::new("media")
        .route_prefix("/media")
        .media(
            "films",
            PLAY,
            Folder {
                files: &[
                    "harbor.webm",
                    "harbor.mp4",
                    "harbor.m3u8",
                    "harbor0.ts",
                    "harbor1.ts",
                    "harbor.jpg",
                    "harbor.en.vtt",
                    "harbor.tr.vtt",
                    "zoom.mp4",
                    "zoom.jpg",
                ],
                positions: Some(Arc::default()),
            },
        )
        .media(
            "music",
            PLAY,
            Folder { files: &["dawn.mp3", "tide.mp3", "lanterns.mp3", "dawn.en.vtt"], positions: None },
        )
        .screen(media_library)
        .screen(media_play)
        .screen(media_external)
        .menu(|m| {
            m.group(t!("menu.media"), Icon::Chart)
                .item(t!("media.title"), media_library)
                .item(t!("media.external"), media_external)
        })
}
