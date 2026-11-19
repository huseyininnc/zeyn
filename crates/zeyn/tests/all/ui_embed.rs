//! Content from other sites: `Embed` frames, the `#[rule(embed)]` field rule, `Image`,
//! `ExternalLink`, and the content security policy they read.

use axum::body::Body;
use axum::http::{Request, header};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    EMBED_SANDBOX, Embed, EmbedPermission, ExternalLink, Fit, Image, PLACEHOLDER_BLUR_MAX,
    Placeholder,
};
use zeyn::{
    Choice, ClientPref, ClientPrefs, Component, Consent, Csp, Identity, Purpose, Render, Rule,
    Services, embed_url,
};

fn ctx(csp: Csp) -> Ctx {
    Ctx::for_tests(Identity::anonymous(), Services::default()).with_csp(Arc::new(csp))
}

fn frames() -> Csp {
    Csp::new()
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .img_src(["https://i.example.com"])
}

fn html(c: &impl Component, ctx: &Ctx) -> String {
    let mut out = String::new();
    c.render(&mut out, &Render::english("s").with_ctx(ctx));
    out
}

#[test]
fn embed_on_an_allowed_host_is_a_locked_lazy_frame() {
    let e = Embed::url("https://www.youtube-nocookie.com/embed/abc?utm_source=x&t=4")
        .title("Product tour")
        .allow([
            EmbedPermission::Fullscreen,
            EmbedPermission::PictureInPicture,
        ])
        .aspect(4, 3);
    let out = html(&e, &ctx(frames()));
    assert!(
        out.contains(
            r#"<iframe src="https://www.youtube-nocookie.com/embed/abc?t=4" title="Product tour""#
        ),
        "{out}"
    );
    assert!(out.contains(&format!(r#"sandbox="{EMBED_SANDBOX}""#)));
    assert_eq!(
        EMBED_SANDBOX,
        "allow-scripts allow-same-origin allow-presentation"
    );
    assert!(out.contains(r#"allow="fullscreen; picture-in-picture""#));
    assert!(out.contains(r#"referrerpolicy="no-referrer" loading="lazy""#));
    assert!(out.contains("--z-aspect: 4 / 3"));
    assert!(
        !out.contains("allow-popups")
            && !out.contains("allow-top-navigation")
            && !out.contains("allow-forms")
    );
    assert!(
        html(
            &Embed::url("https://player.vimeo.com/video/1").title("V"),
            &ctx(frames())
        )
        .contains("<iframe")
    );
}

#[test]
fn embed_on_an_unlisted_host_renders_no_frame() {
    for url in [
        "https://evil.example.com/embed/abc",
        "https://vimeo.com/1",
        "http://www.youtube-nocookie.com/embed/abc",
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "<script>alert(1)</script>",
        "https://www.youtube-nocookie.com@evil.example.com/",
    ] {
        let out = html(&Embed::url(url).title("Tour"), &ctx(frames()));
        assert!(
            !out.contains("<iframe") && !out.contains("<template"),
            "{url}: {out}"
        );
        assert!(
            out.contains("z-callout z-tone-warning") && out.contains("Tour cannot be shown here"),
            "{url}: {out}"
        );
        assert!(
            !out.contains("evil") && !out.contains("<script"),
            "{url}: {out}"
        );
    }
    let outside_a_request = html(
        &Embed::url("https://www.youtube-nocookie.com/embed/abc").title("T"),
        &Ctx::for_tests(Identity::anonymous(), Services::default()),
    );
    assert!(
        !outside_a_request.contains("<iframe"),
        "no policy means no hosts"
    );
}

#[test]
fn click_to_load_shows_a_poster_button_and_keeps_the_frame_inert() {
    let e = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Tour")
        .click_to_load("https://i.example.com/p.jpg");
    let out = html(&e, &ctx(frames()));
    let button = out
        .find(r#"<button type="button" class="z-embed-load" data-zeyn-embed-load>"#)
        .expect(&out);
    let template = out
        .find("<template data-zeyn-embed><iframe")
        .expect(&out);
    assert!(button < template);
    assert!(out.contains(r#"<img src="https://i.example.com/p.jpg" alt="""#));
    assert!(
        out.contains("<strong>Load Tour</strong>")
            && out.contains("Content from www.youtube-nocookie.com")
    );
    let unlisted_poster = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Tour")
        .click_to_load("https://other.test/p.jpg");
    assert!(
        !html(&unlisted_poster, &ctx(frames())).contains("other.test"),
        "a poster from an unlisted host is left out"
    );
}

#[test]
fn a_notice_on_the_gate_is_escaped_translated_and_part_of_the_button_name() {
    let e = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Product tour")
        .click_to_load("")
        .notice("Sound <b>on</b> & autoplay");
    let out = html(&e, &ctx(frames()));
    let button = &out[out.find("<button").expect(&out)..out.find("</button>").expect(&out)];
    assert!(
        button.contains(r#"<span class="z-embed-notice">Sound &lt;b&gt;on&lt;/b&gt; &amp; autoplay</span><strong>Load Product tour</strong>"#),
        "{out}"
    );
    assert!(!out.contains("<b>"), "{out}");
    assert_eq!(e.schema()["notice"], "Sound <b>on</b> & autoplay");

    let without_poster = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("T")
        .notice("Warning");
    let out = html(&without_poster, &ctx(frames()));
    assert!(
        out.contains("data-zeyn-embed-load") && out.contains("<template data-zeyn-embed><iframe"),
        "{out}"
    );
    assert!(!out.contains("<img"), "{out}");

    let tr = zeyn::I18n::builtin();
    let translated = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Ürün turu")
        .notice(Text::key("zeyn.ui.link.external"));
    let mut out = String::new();
    translated.render(
        &mut out,
        &Render::new("s", "tr", tr).with_ctx(&ctx(frames())),
    );
    assert!(
        out.contains(r#"<span class="z-embed-notice">dış site</span>"#),
        "{out}"
    );
}

const NOTICE: ClientPref<Consent> =
    ClientPref::consent("external_video-2", DAY, Purpose::Necessary);

#[test]
fn a_remembered_gate_names_its_key_and_keeps_the_frame_inert() {
    let e = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Product tour")
        .notice("Served by another site")
        .remember(NOTICE);
    let mut services = Services::default();
    services.insert(ClientPrefs::default().key(NOTICE));
    let registered = Ctx::for_tests(Identity::anonymous(), services).with_csp(Arc::new(frames()));
    let out = html(&e, &registered);
    assert!(out.starts_with(r#"<div class="z-embed" style="--z-aspect: 16 / 9" data-zeyn-remember="external_video-2" data-zeyn-remember-for="86400000" data-zeyn-remember-purpose="necessary""#), "{out}");
    assert!(
        out.contains("data-zeyn-embed-load") && out.contains("<template data-zeyn-embed><iframe"),
        "{out}"
    );
    assert!(
        !out.replace("<template data-zeyn-embed><iframe", "")
            .contains("<iframe"),
        "without JavaScript the frame stays inert: {out}"
    );
    assert_eq!(
        e.schema()["remember"],
        serde_json::json!({ "name": "external_video-2", "kind": "consent", "purpose": "necessary", "lifetime": 86_400 })
    );
    let unregistered = html(&e, &ctx(frames()));
    assert!(
        !unregistered.contains("data-zeyn-remember")
            && unregistered.contains("data-zeyn-embed-load"),
        "a key the app did not register is not remembered: {unregistered}"
    );
    let plain = html(
        &Embed::url("https://www.youtube-nocookie.com/embed/abc")
            .title("T")
            .click_to_load(""),
        &registered,
    );
    assert!(!plain.contains("data-zeyn-remember"), "{plain}");
}

#[test]
fn embed_urls_are_canonical_and_never_html() {
    let pasted = r#"<iframe width="560" height="315" src="https://WWW.YouTube-nocookie.com/embed/abc?si=x&amp;utm_source=share&amp;fbclid=1" title="YouTube video player" frameborder="0" allow="autoplay" allowfullscreen></iframe>"#;
    assert_eq!(
        embed_url(pasted).as_deref(),
        Some("https://www.youtube-nocookie.com/embed/abc?si=x")
    );
    assert_eq!(
        embed_url(" https://a.test:443/x?gclid=2&utm_medium=m#t=1 ").as_deref(),
        Some("https://a.test/x#t=1")
    );
    for bad in [
        "javascript:alert(1)",
        "JAVASCRIPT:alert(1)",
        "<script src=\"https://a.test/x.js\"></script>",
        "<iframe src=\"javascript:alert(1)\"></iframe>",
        "http://a.test/",
        "https://a.test/x\"onload=\"alert(1)",
        "https://user:pw@a.test/",
        "//a.test/",
        "",
    ] {
        assert_eq!(embed_url(bad), None, "{bad}");
    }
}

#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}

#[action(public)]
pub async fn save_video(v: Valid<Video>) -> Result<Video> {
    Ok(v.into_inner())
}

fn video_form() -> zeyn::SubmitForm<Video, save_video> {
    Form::of(Video::default())
        .field(Video::url)
        .submit("Save", save_video)
}

#[test]
fn embed_rule_stores_the_canonical_address_of_pasted_embed_code() {
    let ctx = ctx(frames());
    let values = serde_json::json!({ "url": r#"<iframe src="https://www.youtube-nocookie.com/embed/abc?utm_campaign=c&amp;t=9"></iframe>"# });
    let v = video_form()
        .form()
        .read_for(values.as_object().unwrap(), &ctx)
        .unwrap();
    assert_eq!(v.url, "https://www.youtube-nocookie.com/embed/abc?t=9");

    let unlisted = serde_json::json!({ "url": "https://evil.example.com/embed/abc" });
    let errors = video_form()
        .form()
        .read_for(unlisted.as_object().unwrap(), &ctx)
        .unwrap_err();
    assert_eq!(
        errors
            .errors
            .iter()
            .map(|e| (e.field, e.rule))
            .collect::<Vec<_>>(),
        [("url", Rule::Embed)]
    );
    assert!(
        ctx.t(&errors.errors[0].message)
            .contains("evil.example.com is not on the list"),
        "{errors:?}"
    );

    for bad in [
        "javascript:alert(1)",
        "<script>alert(1)</script>",
        "http://www.youtube-nocookie.com/embed/abc",
    ] {
        let values = serde_json::json!({ "url": bad });
        let errors = video_form()
            .form()
            .read_for(values.as_object().unwrap(), &ctx)
            .unwrap_err();
        assert!(
            errors
                .errors
                .iter()
                .any(|e| e.field == "url" && e.rule == Rule::Embed),
            "{bad}: {errors:?}"
        );
    }
}

#[test]
fn embed_rule_refuses_html_outside_forms() {
    let html = Video {
        url: r#"<iframe src="https://www.youtube-nocookie.com/embed/abc"></iframe>"#.into(),
        ..Video::default()
    };
    let rules: Vec<Rule> = Valid::new(html)
        .unwrap_err()
        .errors
        .into_iter()
        .map(|e| e.rule)
        .collect();
    assert_eq!(rules, [Rule::Embed]);
    assert!(
        Valid::new(Video {
            url: "https://www.youtube-nocookie.com/embed/abc".into(),
            ..Video::default()
        })
        .is_ok()
    );
}

#[tokio::test]
async fn api_inputs_with_an_unlisted_embed_host_are_refused() {
    let router = App::new("T")
        .csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))
        .auth(|_: &axum::http::HeaderMap| Some(Identity::user("ayse")))
        .feature(Feature::new("media").api(save_video))
        .router()
        .unwrap();
    let call = |url: &str| {
        Request::post("/api/v1/media/save_video")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, "Bearer test")
            .body(Body::from(
                serde_json::json!({ "id": 1, "url": url }).to_string(),
            ))
            .unwrap()
    };
    let ok = router
        .clone()
        .oneshot(call("https://www.youtube-nocookie.com/embed/abc"))
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    let refused = router
        .clone()
        .oneshot(call("https://evil.example.com/embed/abc"))
        .await
        .unwrap();
    assert_ne!(refused.status(), 200);
    let body = String::from_utf8(
        refused
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("evil.example.com"), "{body}");
}

#[test]
fn images_render_with_size_alt_and_async_decoding() {
    let img = Image::new("https://i.example.com/logo.png")
        .alt("Northwind logo")
        .size(240, 80)
        .lazy()
        .srcset([
            ("https://i.example.com/logo@2x.png", "2x"),
            ("https://other.test/logo@3x.png", "3x"),
            (
                "https://i.example.com/a.png",
                "2x, https://other.test/b.png 3x",
            ),
            ("https://i.example.com/w.png", "800w"),
        ]);
    let out = html(&img, &ctx(frames()));
    assert!(out.starts_with(r#"<img class="z-image" src="https://i.example.com/logo.png" alt="Northwind logo" width="240" height="80" decoding="async" loading="lazy""#), "{out}");
    assert!(out.contains(r#"srcset="https://i.example.com/logo@2x.png 2x, https://i.example.com/w.png 800w" sizes="auto, (max-width: 240px) 100vw, 240px""#), "{out}");
    assert!(!out.contains("other.test"), "{out}");
    let local = html(
        &Image::new("/static/logo.png")
            .size(10, 10)
            .alt("Logo"),
        &ctx(Csp::new()),
    );
    assert!(
        local.contains(r#"src="/static/logo.png""#)
            && !local.contains("loading=")
            && !local.contains("srcset")
    );
}

const TINY: &str = "data:image/webp;base64,UklGRhoAAABXRUJQVlA4TA0AAAAvAAAAEAcQERGIiP4HAA==";

#[test]
fn progressive_images_sit_in_a_reserved_box_with_their_placeholder_low_file_and_fallback() {
    let img = Image::new("https://i.example.com/p/1-800.webp")
        .alt("Blue <shirt>")
        .size(400, 500)
        .lazy()
        .srcset([
            ("https://i.example.com/p/1-400.webp", "400w"),
            ("https://i.example.com/p/1-800.webp", "800w"),
        ])
        .sizes("(min-width: 800px) 25vw, 50vw")
        .placeholder(Placeholder::blur(TINY))
        .low("/p/1-40.webp")
        .fallback("/p/none.webp")
        .on_error(Text::from("No <photo>"))
        .fit(Fit::Cover);
    let out = html(&img, &ctx(frames()));
    assert_eq!(
        out,
        format!(
            r#"<span class="z-img z-img-host z-img-blur z-img-cover" style="width:400px;aspect-ratio:400 / 500;--z-ph-img:url({TINY})"><img class="z-img-under" src="/p/1-40.webp" alt="" decoding="async" loading="lazy"><img class="z-image" src="https://i.example.com/p/1-800.webp" alt="Blue &lt;shirt&gt;" width="400" height="500" decoding="async" loading="lazy" data-zeyn-img data-zeyn-fallback="/p/none.webp" srcset="https://i.example.com/p/1-400.webp 400w, https://i.example.com/p/1-800.webp 800w" sizes="(min-width: 800px) 25vw, 50vw" data-testid="s.image"><template data-zeyn-img-error>No &lt;photo&gt;</template></span>"#
        )
    );
}

#[test]
fn the_first_image_loads_at_once_and_a_plain_image_keeps_its_markup() {
    let first = html(
        &Image::new("/hero.webp")
            .alt("Hero")
            .size(1200, 600)
            .lazy()
            .priority(),
        &ctx(Csp::new()),
    );
    assert_eq!(
        first,
        r#"<img class="z-image" src="/hero.webp" alt="Hero" width="1200" height="600" decoding="async" loading="eager" fetchpriority="high" data-testid="s.image">"#
    );
    let plain = html(
        &Image::new("/a.webp")
            .alt("A")
            .size(10, 10),
        &ctx(Csp::new()),
    );
    assert!(!plain.contains("z-img") && !plain.contains("data-zeyn-img"));
}

#[test]
fn placeholders_and_extra_files_follow_the_image_rules() {
    let long = format!("data:image/png;base64,{}", "A".repeat(PLACEHOLDER_BLUR_MAX));
    for bad in [
        Placeholder::blur("data:image/svg+xml;base64,PHN2ZyBvbmxvYWQ9YWxlcnQoMSk+"),
        Placeholder::blur("data:image/svg+xml,<svg onload=alert(1)>"),
        Placeholder::blur(&long),
        Placeholder::blur("data:image/png;base64,AAA)\" onload=x"),
        Placeholder::blur("https://i.example.com/tiny.png"),
        Placeholder::color("red;background:url(https://other.test/x)"),
        Placeholder::color("#12345"),
        Placeholder::color("--z-Surface;x"),
        Placeholder::image("https://other.test/outline.png"),
    ] {
        let out = html(
            &Image::new("/a.webp")
                .alt("A")
                .size(10, 10)
                .placeholder(bad.clone()),
            &ctx(frames()),
        );
        assert!(
            out.starts_with(r#"<span class="z-img z-img-host" style="width:10px;aspect-ratio:10 / 10"><img class="z-image""#),
            "{bad:?} is left out: {out}"
        );
    }
    let out = html(
        &Image::new("/a.webp")
            .alt("A")
            .size(10, 10)
            .placeholder(Placeholder::image("https://i.example.com/outline.png"))
            .low("javascript:alert(1)")
            .fallback("//other.test/x.png")
            .sizes(r#"50vw" onerror="alert(1)"#)
            .srcset([("/a-20.webp", "20w")]),
        &ctx(frames()),
    );
    assert!(
        out.contains(r#"<img class="z-img-under" src="https://i.example.com/outline.png" alt="""#)
    );
    assert!(!out.contains("javascript") && !out.contains("other.test") && !out.contains("onerror"));
    assert!(
        out.contains(r#"sizes="(max-width: 10px) 100vw, 10px""#),
        "{out}"
    );
    let token = html(
        &Image::new("/a.webp")
            .alt("A")
            .size(10, 10)
            .placeholder(Placeholder::color("--z-surface-2")),
        &ctx(Csp::new()),
    );
    assert!(token.contains("--z-ph:var(--z-surface-2)"));
}

#[test]
fn images_from_unlisted_hosts_show_their_alt_text() {
    for src in [
        "https://other.test/x.png",
        "http://i.example.com/x.png",
        "javascript:alert(1)",
        "//other.test/x.png",
        "data:image/svg+xml,<svg onload=alert(1)>",
    ] {
        let out = html(
            &Image::new(src)
                .alt("A <b>chart</b>")
                .size(10, 10),
            &ctx(frames()),
        );
        assert_eq!(
            out,
            r#"<span class="z-image-blocked" data-testid="s.image">A &lt;b&gt;chart&lt;/b&gt;</span>"#,
            "{src}"
        );
    }
}

#[test]
fn external_links_never_hand_over_the_window() {
    let r = Render::english("s");
    let plain = ExternalLink::new("https://example.com/help", "Help").html(&r);
    assert_eq!(
        plain,
        r#"<a class="z-link-external" href="https://example.com/help" rel="noopener noreferrer" data-testid="s.link">Help<span class="z-sr-only"> (external site)</span><span class="z-link-external-icon" aria-hidden="true">&#8599;</span></a>"#
    );
    let user = ExternalLink::new("https://example.org/?a=1&b=2", "Site")
        .user_content()
        .new_tab()
        .html(&r);
    assert!(
        user.contains(r#"href="https://example.org/?a=1&amp;b=2""#),
        "{user}"
    );
    assert!(
        user.contains(r#"rel="noopener noreferrer nofollow ugc" target="_blank""#),
        "{user}"
    );
    assert!(user.contains("(external site, opens in a new tab)"));
    for bad in [
        "javascript:alert(1)",
        "data:text/html,x",
        "/local",
        "https://",
        "https://a b.test",
    ] {
        let out = ExternalLink::new(bad, "x").html(&r);
        assert_eq!(out, r#"<span class="z-link-text">x</span>"#, "{bad}");
    }
    let tr = zeyn::I18n::builtin();
    let out = ExternalLink::new("https://example.com", "Yardım")
        .new_tab()
        .html(&Render::new("s", "tr", tr));
    assert!(out.contains("(dış site, yeni sekmede açılır)"), "{out}");
}

const DAY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

#[test]
fn client_prefs_are_listed_for_the_privacy_page_and_keys_registered_twice_are_problems() {
    const TAB: ClientPref<Choice> =
        ClientPref::choice("order-tab", zeyn::MAX_PREF_LIFETIME, Purpose::Preference);
    let app = App::new("Prefs").client_prefs(|p| p.key(NOTICE).key(TAB));
    let table = app.client_prefs_table();
    assert_eq!(
        serde_json::to_value(table).unwrap(),
        serde_json::json!([
            { "name": "external_video-2", "kind": "consent", "purpose": "necessary", "lifetime": 86_400 },
            { "name": "order-tab", "kind": "choice", "purpose": "preference", "lifetime": 400 * 86_400 },
        ])
    );
    let twice = App::new("Twice").client_prefs(|p| p.key(NOTICE).key(NOTICE));
    let problems = twice.check().unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("client preference `external_video-2` is registered twice")),
        "{problems:?}"
    );
}

#[tokio::test]
async fn the_consent_answer_comes_from_the_apps_hook_and_is_never_cached() {
    let app = |hook: bool| {
        App::new("Prefs")
            .client_prefs(|p| {
                let p = p.key(NOTICE);
                if hook {
                    p.allowed_when(|h| {
                        h.get(header::COOKIE)
                            .and_then(|c| c.to_str().ok())
                            .is_some_and(|c| c.contains("consent=all"))
                    })
                } else {
                    p
                }
            })
            .router()
            .unwrap()
    };
    let ask = |router: axum::Router, cookie: &'static str| async move {
        let res = router
            .oneshot(
                Request::get("/_zeyn/prefs")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
        let body = res
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["preferences"].clone()
    };
    assert_eq!(ask(app(true), "consent=all").await, true);
    assert_eq!(ask(app(true), "consent=none").await, false);
    assert_eq!(
        ask(app(false), "consent=all").await,
        false,
        "without a hook preferences stay for the session"
    );
    let js = app(false)
        .oneshot(
            Request::get("/_zeyn/state.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(js.status(), 200);
    let js = js
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let js = String::from_utf8_lossy(&js);
    assert!(js.contains("'zeyn.v1.'") && js.contains("sessionStorage"));
}
