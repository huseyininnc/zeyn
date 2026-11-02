//! Video and audio players: native `<video>` and `<audio>` under custom, keyboard friendly
//! controls, with captions, playback speed and resume.
//!
//! Files come from a media source registered with [`Feature::media`](crate::Feature::media), so the
//! server checks the permission on every request and answers range requests for seeking.

use super::{dom_id, layout::into_node};
use crate::i18n::Text;
use crate::media::{media_type, valid_media_id};
use crate::render::{Render, attr, escape, slug};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Video, Audio);

/// Playback speeds offered in the speed menu.
pub const PLAYBACK_SPEEDS: [&str; 6] = ["0.5", "0.75", "1", "1.25", "1.5", "2"];

/// A captions file of a player.
struct Captions {
    id: String,
    lang: String,
    label: Text,
}

/// What both players share: where the files come from and what to show.
struct Player {
    source: &'static str,
    title: Text,
    subtitle: Option<Text>,
    files: Vec<String>,
    hls: Option<String>,
    dash: Option<String>,
    poster: Option<String>,
    captions: Vec<Captions>,
}

impl Player {
    fn new(source: &'static str, title: Text) -> Self {
        Player {
            source,
            title,
            subtitle: None,
            files: Vec::new(),
            hls: None,
            dash: None,
            poster: None,
            captions: Vec::new(),
        }
    }

    fn url(&self, id: &str) -> String {
        format!("/_zeyn/media/{}/{}", escape(self.source), escape(id))
    }

    /// The id positions are saved under: the first file, else the adaptive playlist.
    fn key(&self) -> Option<&String> {
        self.files.first().or(self.hls.as_ref()).or(self.dash.as_ref())
    }

    fn schema(&self, kind: &str) -> Json {
        json!({
            "type": kind,
            "source": self.source,
            "title": self.title,
            "subtitle": self.subtitle,
            "files": self.files,
            "hls": self.hls,
            "dash": self.dash,
            "poster": self.poster,
            "captions": self.captions.iter().map(|c| json!({ "id": c.id, "lang": c.lang, "label": c.label })).collect::<Vec<_>>(),
        })
    }

    fn render(&self, out: &mut String, r: &Render, video: bool) {
        let kind = if video { "video" } else { "audio" };
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.media.{key}")));
        let title = r.text(&self.title);
        let texts: Json = [
            "play",
            "pause",
            "mute",
            "unmute",
            "time_of",
            "speed_value",
            "fullscreen",
            "exit_fullscreen",
            "resumed",
            "failed",
            "captions_on",
            "captions_off",
        ]
        .iter()
        .map(|k| (k.to_string(), json!(r.i18n.text(r.locale, &Text::key(format!("zeyn.ui.media.{k}"))))))
        .collect::<serde_json::Map<_, _>>()
        .into();
        out.push_str(&format!(
            r#"<div class="z-player z-player-{kind}" data-zeyn-player="{kind}" data-zeyn-media-source="{}" data-zeyn-media-id="{}" data-zeyn-texts="{}" data-testid="{}.{kind}.{}""#,
            escape(self.source),
            escape(self.key().map(String::as_str).unwrap_or("")),
            attr(&texts),
            r.scope,
            slug(&r.i18n.text(r.locale, &self.title)),
        ));
        if let Some(id) = &self.hls {
            out.push_str(&format!(r#" data-zeyn-hls="{}""#, self.url(id)));
        }
        if let Some(id) = &self.dash {
            out.push_str(&format!(r#" data-zeyn-dash="{}""#, self.url(id)));
        }
        out.push('>');
        out.push_str(r#"<div class="z-player-stage">"#);
        let poster =
            self.poster.as_ref().map(|p| format!(r#" data-zeyn-poster="{}""#, self.url(p))).unwrap_or_default();
        out.push_str(&format!(
            r#"<{kind} class="z-player-media" preload="metadata" playsinline aria-label="{title}"{poster}>"#
        ));
        for id in &self.files {
            out.push_str(&format!(
                r#"<source data-zeyn-src="{}" type="{}">"#,
                self.url(id),
                escape(media_type(id).split(';').next().unwrap_or(""))
            ));
        }
        for c in &self.captions {
            out.push_str(&format!(
                r#"<track kind="subtitles" data-zeyn-src="{}" srclang="{}" label="{}">"#,
                self.url(&c.id),
                escape(&c.lang),
                r.text(&c.label)
            ));
        }
        out.push_str(&format!(r#"</{kind}><div class="z-player-captions" data-zeyn-player-captions hidden></div>"#));
        out.push_str("</div>");
        out.push_str(&format!(r#"<div class="z-player-info"><p class="z-player-title">{title}</p>"#));
        if let Some(s) = &self.subtitle {
            out.push_str(&format!(r#"<p class="z-player-subtitle">{}</p>"#, r.text(s)));
        }
        out.push_str("</div>");
        out.push_str(&format!(
            r#"<div class="z-player-controls" role="group" aria-label="{}: {title}">"#,
            t(&format!("{kind}_player"))
        ));
        out.push_str(&format!(
            r#"<button type="button" class="z-player-button" data-zeyn-player-play aria-label="{}"><span class="z-player-icon z-player-i-play" aria-hidden="true"></span></button>"#,
            t("play")
        ));
        out.push_str(&format!(
            r#"<div class="z-player-slider z-player-seek" role="slider" tabindex="0" data-zeyn-player-seek aria-label="{}" aria-valuemin="0" aria-valuemax="0" aria-valuenow="0" aria-valuetext="0:00"><span class="z-player-rail"><span class="z-player-buffered"></span><span class="z-player-fill"></span></span><span class="z-player-thumb"></span></div>"#,
            t("seek")
        ));
        out.push_str(r#"<span class="z-player-time" data-zeyn-player-time aria-hidden="true">0:00 / 0:00</span>"#);
        out.push_str(&format!(
            r#"<button type="button" class="z-player-button" data-zeyn-player-mute aria-label="{}" aria-pressed="false"><span class="z-player-icon z-player-i-volume" aria-hidden="true"></span></button>"#,
            t("mute")
        ));
        out.push_str(&format!(
            r#"<div class="z-player-slider z-player-volume" role="slider" tabindex="0" data-zeyn-player-volume aria-label="{}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="100" aria-valuetext="100%"><span class="z-player-rail"><span class="z-player-fill"></span></span><span class="z-player-thumb"></span></div>"#,
            t("volume")
        ));
        let speed_id = dom_id(r, "speed");
        let speed_label = r.i18n.text(r.locale, &Text::key("zeyn.ui.media.speed_value").arg("value", "1×"));
        out.push_str(&format!(
            r#"<div class="z-player-menu"><button type="button" class="z-player-button z-player-text" data-zeyn-player-menu aria-haspopup="menu" aria-expanded="false" aria-controls="{speed_id}" aria-label="{}" data-zeyn-player-speed>1×</button><ul class="z-player-list" role="menu" id="{speed_id}" aria-label="{}" hidden>"#,
            escape(&speed_label),
            t("speed")
        ));
        for s in PLAYBACK_SPEEDS {
            out.push_str(&format!(
                r#"<li role="menuitemradio" tabindex="-1" data-zeyn-speed="{s}" aria-checked="{}">{s}×</li>"#,
                s == "1"
            ));
        }
        out.push_str("</ul></div>");
        if !self.captions.is_empty() {
            out.push_str(&format!(
                r#"<button type="button" class="z-player-button z-player-text" data-zeyn-player-cc aria-pressed="false" aria-label="{}">CC</button>"#,
                t("captions")
            ));
        }
        if self.captions.len() > 1 {
            let cc_id = dom_id(r, "captions");
            out.push_str(&format!(
                r#"<div class="z-player-menu"><button type="button" class="z-player-button" data-zeyn-player-menu aria-haspopup="menu" aria-expanded="false" aria-controls="{cc_id}" aria-label="{}"><span class="z-player-icon z-player-i-language" aria-hidden="true"></span></button><ul class="z-player-list" role="menu" id="{cc_id}" aria-label="{}" hidden>"#,
                t("captions_language"),
                t("captions_language")
            ));
            out.push_str(&format!(
                r#"<li role="menuitemradio" tabindex="-1" data-zeyn-track="-1" aria-checked="true">{}</li>"#,
                t("captions_off")
            ));
            for (i, c) in self.captions.iter().enumerate() {
                out.push_str(&format!(
                    r#"<li role="menuitemradio" tabindex="-1" data-zeyn-track="{i}" aria-checked="false">{}</li>"#,
                    r.text(&c.label)
                ));
            }
            out.push_str("</ul></div>");
        }
        if video {
            out.push_str(&format!(
                r#"<button type="button" class="z-player-button" data-zeyn-player-pip aria-label="{}" aria-pressed="false" hidden><span class="z-player-icon z-player-i-pip" aria-hidden="true"></span></button>"#,
                t("pip")
            ));
            out.push_str(&format!(
                r#"<button type="button" class="z-player-button" data-zeyn-player-fullscreen aria-label="{}"><span class="z-player-icon z-player-i-fullscreen" aria-hidden="true"></span></button>"#,
                t("fullscreen")
            ));
        }
        out.push_str("</div>");
        out.push_str(&format!(
            r#"<p class="z-player-error" data-zeyn-player-error hidden>{}</p><p class="z-sr-only" role="status" data-zeyn-player-status></p></div>"#,
            t("failed")
        ));
    }
}

macro_rules! player_builder {
    ($t:ident) => {
        impl $t {
            /// A file to play, by its id in the media source. Add the same media in several
            /// formats (`.webm`, `.mp4`) in order of preference; the browser plays the first one
            /// it supports. Saved positions are kept under the first file's id.
            pub fn file(mut self, id: impl Into<String>) -> Self {
                let id = id.into();
                if valid_media_id(&id) {
                    self.0.files.push(id);
                }
                self
            }

            /// An HLS playlist (`.m3u8`). Browsers that play HLS themselves (Safari) use it;
            /// others play the files, unless an adaptive streaming library takes it over through
            /// the `zeyn:media-adaptive` event (see `docs/media.md`).
            pub fn hls(mut self, id: impl Into<String>) -> Self {
                self.0.hls = Some(id.into()).filter(|i| valid_media_id(i));
                self
            }

            /// A DASH manifest (`.mpd`). No browser plays DASH itself: it is used only when an
            /// adaptive streaming library takes it over through the `zeyn:media-adaptive` event.
            pub fn dash(mut self, id: impl Into<String>) -> Self {
                self.0.dash = Some(id.into()).filter(|i| valid_media_id(i));
                self
            }

            /// A captions file in WebVTT, its language (`en`, `tr`) and its name in the menu.
            pub fn captions(mut self, id: impl Into<String>, lang: &str, label: impl Into<Text>) -> Self {
                let id = id.into();
                if valid_media_id(&id) {
                    self.0.captions.push(Captions { id, lang: lang.to_string(), label: label.into() });
                }
                self
            }

            /// A second line under the title, such as the artist or the season.
            pub fn subtitle(mut self, text: impl Into<Text>) -> Self {
                self.0.subtitle = Some(text.into());
                self
            }
        }
    };
}

/// A video player: native `<video>` with custom controls for play, seek, time, volume, mute,
/// speed, captions, picture in picture and full screen, and resume where the user stopped.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::Component;
///
/// let v = Video::new("films", "Harbor at dawn").file("harbor.webm").file("harbor.mp4").captions("harbor.en.vtt", "en", "English");
/// let mut html = String::new();
/// v.render(&mut html, &zeyn::Render::english("s"));
/// assert!(html.contains(r#"data-zeyn-src="/_zeyn/media/films/harbor.webm""#));
/// assert!(html.contains(r#"role="slider""#));
/// ```
pub struct Video(Player);

impl Video {
    /// A player for files of the media source `source`, with a title for its label.
    pub fn new(source: &'static str, title: impl Into<Text>) -> Self {
        Video(Player::new(source, title.into()))
    }

    /// An image shown before playing, by its id in the media source.
    pub fn poster(mut self, id: impl Into<String>) -> Self {
        self.0.poster = Some(id.into()).filter(|i| valid_media_id(i));
        self
    }
}

player_builder!(Video);

impl Component for Video {
    fn render(&self, out: &mut String, r: &Render) {
        self.0.render(out, r, true);
    }

    fn schema(&self) -> Json {
        self.0.schema("video")
    }
}

/// An audio player: native `<audio>` with the same controls as [`Video`] except picture in
/// picture and full screen. Captions (lyrics, transcripts) show under the title.
pub struct Audio(Player);

impl Audio {
    pub fn new(source: &'static str, title: impl Into<Text>) -> Self {
        Audio(Player::new(source, title.into()))
    }
}

player_builder!(Audio);

impl Component for Audio {
    fn render(&self, out: &mut String, r: &Render) {
        self.0.render(out, r, false);
    }

    fn schema(&self) -> Json {
        self.0.schema("audio")
    }
}
