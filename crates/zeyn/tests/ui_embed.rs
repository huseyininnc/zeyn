//! Content from other sites: `Embed` frames, the `#[rule(embed)]` field rule, `Image`,
//! `ExternalLink`, and the content security policy they read.

use axum::body::Body;
use axum::http::{Request, header};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{EMBED_SANDBOX, Embed, EmbedPermission, ExternalLink, Image};
use zeyn::{Component, Csp, Identity, Render, Rule, Services, embed_url};

fn ctx(csp: Csp) -> Ctx {
    Ctx::for_tests(Identity::anonymous(), Services::default()).with_csp(Arc::new(csp))
}

fn frames() -> Csp {
    Csp::new().frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"]).img_src(["https://i.example.com"])
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
        .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
        .aspect(4, 3);
    let out = html(&e, &ctx(frames()));
    assert!(
        out.contains(r#"<iframe src="https://www.youtube-nocookie.com/embed/abc?t=4" title="Product tour""#),
        "{out}"
    );
    assert!(out.contains(&format!(r#"sandbox="{EMBED_SANDBOX}""#)));
    assert_eq!(EMBED_SANDBOX, "allow-scripts allow-same-origin allow-presentation");
    assert!(out.contains(r#"allow="fullscreen; picture-in-picture""#));
    assert!(out.contains(r#"referrerpolicy="no-referrer" loading="lazy""#));
    assert!(out.contains("--z-aspect: 4 / 3"));
    assert!(!out.contains("allow-popups") && !out.contains("allow-top-navigation") && !out.contains("allow-forms"));
    assert!(html(&Embed::url("https://player.vimeo.com/video/1").title("V"), &ctx(frames())).contains("<iframe"));
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
        assert!(!out.contains("<iframe") && !out.contains("<template"), "{url}: {out}");
        assert!(out.contains("z-callout z-tone-warning") && out.contains("Tour cannot be shown here"), "{url}: {out}");
        assert!(!out.contains("evil") && !out.contains("<script"), "{url}: {out}");
    }
    let outside_a_request = html(
        &Embed::url("https://www.youtube-nocookie.com/embed/abc").title("T"),
        &Ctx::for_tests(Identity::anonymous(), Services::default()),
    );
    assert!(!outside_a_request.contains("<iframe"), "no policy means no hosts");
}

#[test]
fn click_to_load_shows_a_poster_button_and_keeps_the_frame_inert() {
    let e = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Tour")
        .click_to_load("https://i.example.com/p.jpg");
    let out = html(&e, &ctx(frames()));
    let button = out.find(r#"<button type="button" class="z-embed-load" data-zeyn-embed-load>"#).expect(&out);
    let template = out.find("<template data-zeyn-embed><iframe").expect(&out);
    assert!(button < template);
    assert!(out.contains(r#"<img src="https://i.example.com/p.jpg" alt="""#));
    assert!(out.contains("<strong>Load Tour</strong>") && out.contains("Content from www.youtube-nocookie.com"));
    let unlisted_poster = Embed::url("https://www.youtube-nocookie.com/embed/abc")
        .title("Tour")
        .click_to_load("https://other.test/p.jpg");
    assert!(
        !html(&unlisted_poster, &ctx(frames())).contains("other.test"),
        "a poster from an unlisted host is left out"
    );
}

#[test]
fn embed_urls_are_canonical_and_never_html() {
    let pasted = r#"<iframe width="560" height="315" src="https://WWW.YouTube-nocookie.com/embed/abc?si=x&amp;utm_source=share&amp;fbclid=1" title="YouTube video player" frameborder="0" allow="autoplay" allowfullscreen></iframe>"#;
    assert_eq!(embed_url(pasted).as_deref(), Some("https://www.youtube-nocookie.com/embed/abc?si=x"));
    assert_eq!(embed_url(" https://a.test:443/x?gclid=2&utm_medium=m#t=1 ").as_deref(), Some("https://a.test/x#t=1"));
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
    Form::of(Video::default()).field(Video::url).submit("Save", save_video)
}

#[test]
fn embed_rule_stores_the_canonical_address_of_pasted_embed_code() {
    let ctx = ctx(frames());
    let values = serde_json::json!({ "url": r#"<iframe src="https://www.youtube-nocookie.com/embed/abc?utm_campaign=c&amp;t=9"></iframe>"# });
    let v = video_form().form().read_for(values.as_object().unwrap(), &ctx).unwrap();
    assert_eq!(v.url, "https://www.youtube-nocookie.com/embed/abc?t=9");

    let unlisted = serde_json::json!({ "url": "https://evil.example.com/embed/abc" });
    let errors = video_form().form().read_for(unlisted.as_object().unwrap(), &ctx).unwrap_err();
    assert_eq!(errors.errors.iter().map(|e| (e.field, e.rule)).collect::<Vec<_>>(), [("url", Rule::Embed)]);
    assert!(ctx.t(&errors.errors[0].message).contains("evil.example.com is not on the list"), "{errors:?}");

    for bad in ["javascript:alert(1)", "<script>alert(1)</script>", "http://www.youtube-nocookie.com/embed/abc"] {
        let values = serde_json::json!({ "url": bad });
        let errors = video_form().form().read_for(values.as_object().unwrap(), &ctx).unwrap_err();
        assert!(errors.errors.iter().any(|e| e.field == "url" && e.rule == Rule::Embed), "{bad}: {errors:?}");
    }
}

#[test]
fn embed_rule_refuses_html_outside_forms() {
    let html = Video {
        url: r#"<iframe src="https://www.youtube-nocookie.com/embed/abc"></iframe>"#.into(),
        ..Video::default()
    };
    let rules: Vec<Rule> = Valid::new(html).unwrap_err().errors.into_iter().map(|e| e.rule).collect();
    assert_eq!(rules, [Rule::Embed]);
    assert!(Valid::new(Video { url: "https://www.youtube-nocookie.com/embed/abc".into(), ..Video::default() }).is_ok());
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
            .body(Body::from(serde_json::json!({ "id": 1, "url": url }).to_string()))
            .unwrap()
    };
    let ok = router.clone().oneshot(call("https://www.youtube-nocookie.com/embed/abc")).await.unwrap();
    assert_eq!(ok.status(), 200);
    let refused = router.clone().oneshot(call("https://evil.example.com/embed/abc")).await.unwrap();
    assert_ne!(refused.status(), 200);
    let body = String::from_utf8(refused.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.contains("evil.example.com"), "{body}");
}

#[test]
fn images_render_with_size_alt_and_async_decoding() {
    let img = Image::new("https://i.example.com/logo.png").alt("Northwind logo").size(240, 80).lazy().srcset([
        ("https://i.example.com/logo@2x.png", "2x"),
        ("https://other.test/logo@3x.png", "3x"),
        ("https://i.example.com/a.png", "2x, https://other.test/b.png 3x"),
        ("https://i.example.com/w.png", "800w"),
    ]);
    let out = html(&img, &ctx(frames()));
    assert!(out.starts_with(r#"<img class="z-image" src="https://i.example.com/logo.png" alt="Northwind logo" width="240" height="80" decoding="async" loading="lazy""#), "{out}");
    assert!(out.contains(r#"srcset="https://i.example.com/logo@2x.png 2x, https://i.example.com/w.png 800w" sizes="(max-width: 240px) 100vw, 240px""#), "{out}");
    assert!(!out.contains("other.test"), "{out}");
    let local = html(&Image::new("/static/logo.png").size(10, 10).alt("Logo"), &ctx(Csp::new()));
    assert!(local.contains(r#"src="/static/logo.png""#) && !local.contains("loading=") && !local.contains("srcset"));
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
        let out = html(&Image::new(src).alt("A <b>chart</b>").size(10, 10), &ctx(frames()));
        assert_eq!(
            out, r#"<span class="z-image-blocked" data-testid="s.image">A &lt;b&gt;chart&lt;/b&gt;</span>"#,
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
    let user = ExternalLink::new("https://example.org/?a=1&b=2", "Site").user_content().new_tab().html(&r);
    assert!(user.contains(r#"href="https://example.org/?a=1&amp;b=2""#), "{user}");
    assert!(user.contains(r#"rel="noopener noreferrer nofollow ugc" target="_blank""#), "{user}");
    assert!(user.contains("(external site, opens in a new tab)"));
    for bad in ["javascript:alert(1)", "data:text/html,x", "/local", "https://", "https://a b.test"] {
        let out = ExternalLink::new(bad, "x").html(&r);
        assert_eq!(out, r#"<span class="z-link-text">x</span>"#, "{bad}");
    }
    let tr = zeyn::I18n::builtin();
    let out = ExternalLink::new("https://example.com", "Yardım").new_tab().html(&Render::new("s", "tr", tr));
    assert!(out.contains("(dış site, yeni sekmede açılır)"), "{out}");
}
