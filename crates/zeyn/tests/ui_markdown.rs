//! The Markdown component (feature `markdown`): formatting, and that no payload of the usual XSS
//! lists turns into markup, a script URL or an outside image.

use zeyn::ui::{MAX_MARKDOWN, Markdown};

fn html(md: &str) -> String {
    Markdown::new(md.to_string()).html()
}

/// Nothing in `out` can run script or load from elsewhere.
fn inert(out: &str) {
    let lower = out.to_ascii_lowercase();
    for bad in [
        "<script",
        "<iframe",
        "<svg",
        "<object",
        "<embed",
        "<style",
        "<form",
        "<input",
        "<a href=\"javascript",
    ] {
        assert!(!lower.contains(bad), "{bad} in {out}");
    }
    for bad in [
        "javascript:",
        "vbscript:",
        "data:",
        " on",
        "src=\"//",
        "href=\"//",
        "srcdoc",
    ] {
        let in_attr = lower.split('<').skip(1).any(|tag| {
            tag.split('>')
                .next()
                .unwrap_or("")
                .contains(bad)
        });
        assert!(!in_attr, "{bad} inside a tag in {out}");
    }
}

#[test]
fn blocks_and_inline_marks_render_as_html() {
    let out = Markdown::new(
        "# Title\n\nSome **bold**, *it* and `a<b>`.\n\n## A b\n\n- one\n  more\n- two\n  - deep\n\n1. first\n2. second\n\n> quoted\n\n---\n\n| a | b |\n|---|---|\n| `x|y` | 2 |\n",
    )
    .html();
    assert!(out.starts_with(
        "<h2>Title</h2><p>Some <strong>bold</strong>, <em>it</em> and <code>a&lt;b&gt;</code>.</p>"
    ));
    assert!(
        out.contains("<h3>A b</h3>"),
        "user headings have no id: {out}"
    );
    assert!(
        out.contains("<ul><li>one more</li><li>two<ul><li>deep</li></ul></li></ul>"),
        "{out}"
    );
    assert!(out.contains("<ol><li>first</li><li>second</li></ol>"));
    assert!(out.contains("<blockquote><p>quoted</p></blockquote><hr>"));
    assert!(out.contains("<td><code>x|y</code></td><td>2</td>"));
}

#[test]
fn the_component_wraps_the_html_in_prose_and_keeps_braces_of_plain_strings() {
    use zeyn::Component;
    let mut out = String::new();
    Markdown::new("a {b} *c*".to_string())
        .render(&mut out, &zeyn::render::Render::english("notes"));
    assert_eq!(
        out,
        r#"<div class="z-prose z-markdown" data-testid="notes.markdown"><p>a {b} <em>c</em></p></div>"#
    );
    assert_eq!(Markdown::new("x").trusted().schema()["type"], "markdown");
}

#[test]
fn trusted_text_gets_heading_ids_and_drops_nofollow_ugc() {
    let user = html("[a](https://e.org) [b](/x) [c](mailto:a@e.org)");
    assert!(
        user.contains(r#"<a href="https://e.org" rel="noopener noreferrer nofollow ugc">a</a>"#)
    );
    assert!(user.contains(r#"<a href="/x" rel="noopener noreferrer nofollow ugc">b</a>"#));
    assert!(
        user.contains(r#"<a href="mailto:a@e.org" rel="noopener noreferrer nofollow ugc">c</a>"#)
    );
    let app = Markdown::new("## Same\n\n## Same\n\n[a](https://e.org) [b](guide.md#top)")
        .trusted()
        .html();
    assert!(app.contains(r#"<h3 id="same">Same</h3><h3 id="same-2">Same</h3>"#));
    assert!(app.contains(r#"<a href="https://e.org" rel="noopener noreferrer">a</a>"#));
    assert!(app.contains(r#"<a href="guide.md#top">b</a>"#));
}

#[test]
fn raw_html_and_event_handlers_are_escaped_text() {
    let payloads = [
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "<svg/onload=alert(1)>",
        "<a href=\"javascript:alert(1)\">x</a>",
        "<iframe srcdoc=\"<script>alert(1)</script>\">",
        "<scr<script>ipt>alert(1)</script>",
        "\"><script>alert(1)</script>",
        "<style>body{display:none}</style>",
        "<<script>>alert(1)<</script>>",
        "&lt;script&gt;alert(1)&lt;/script&gt;",
    ];
    for p in payloads {
        let out = html(p);
        inert(&out);
        assert!(!out.contains('<') || out.starts_with("<p>"), "{out}");
        assert_eq!(
            out.matches('<').count(),
            2,
            "only the paragraph tags in {out}"
        );
    }
    assert_eq!(
        html("&lt;b&gt;"),
        "<p>&amp;lt;b&amp;gt;</p>",
        "entities stay text"
    );
}

#[test]
fn unsafe_link_targets_keep_only_their_label() {
    let targets = [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "java\tscript:alert(1)",
        " javascript:alert(1)",
        "vbscript:msgbox(1)",
        "data:text/html,<script>alert(1)</script>",
        "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
        "//evil.example/x",
        "/\\evil.example/x",
        "\\\\evil.example",
        "file:///etc/passwd",
        "jav&#x61;script:alert(1)",
        "javascript&colon;alert(1)",
        "javascript&#58alert(1)",
    ];
    for t in targets {
        let out = html(&format!("[click]({t})"));
        inert(&out);
        assert!(!out.contains("<a"), "{t} became a link: {out}");
        assert!(out.contains("click"));
    }
    assert!(
        html("[q](/search?a=1&b=2)").contains(r#"href="/search?a=1&amp;b=2""#),
        "a query may hold &"
    );
    let out = html("[x](https://e.org/\"onmouseover=\"alert(1))");
    inert(&out);
    let quoted = html("[x](https://e.org/a\"b)");
    assert!(
        quoted.contains(r#"href="https://e.org/a&quot;b""#),
        "{quoted}"
    );
}

#[test]
fn nested_tricks_do_not_break_out() {
    let tricks = [
        "[<img src=x onerror=alert(1)>](https://e.org)",
        "[x](https://e.org)<script>alert(1)</script>",
        "**<script>alert(1)</script>**",
        "*[a](javascript:alert(1))*",
        "[[a](javascript:alert(1))](https://e.org)",
        "`<script>`[a](javascript:x)",
        "![<script>](javascript:alert(1))",
        "[a](https://e.org \"title\" onclick=\"alert(1)\")",
        "| <script> | [x](javascript:y) |\n|---|---|\n| a | b |",
        "> <script>alert(1)</script>",
        "- [x](javascript:alert(1))\n  - <img onerror=alert(1)>",
        "# <script>alert(1)</script>",
        "```\n</code></pre><script>alert(1)</script>\n```",
        "```\"><script>alert(1)</script>\nx\n```",
    ];
    for t in tricks {
        inert(&html(t));
        inert(&Markdown::new(t).trusted().html());
    }
}

#[test]
fn code_blocks_are_escaped_and_their_language_is_cleaned() {
    let out = html("```rust onclick=x\nlet s = \"<script>\";\n```");
    assert_eq!(
        out,
        r#"<pre class="z-code" tabindex="0"><code class="language-rustonclickx">let s = &quot;&lt;script&gt;&quot;;</code></pre>"#
    );
}

#[test]
fn images_load_only_from_this_app() {
    assert_eq!(
        html("![Logo](/static/logo.png)"),
        r#"<p><img src="/static/logo.png" alt="Logo" loading="lazy"></p>"#
    );
    for src in [
        "https://tracker.example/p.gif",
        "//cdn.example/x.png",
        "data:image/png;base64,AAAA",
        "javascript:x",
    ] {
        let out = html(&format!("![a \"b\"]({src})"));
        assert_eq!(out, "<p>a &quot;b&quot;</p>", "{src}");
    }
}

#[test]
fn long_and_hostile_input_is_capped_and_stays_fast() {
    let long = "a".repeat(MAX_MARKDOWN + 100);
    assert_eq!(html(&long).len(), MAX_MARKDOWN + "<p></p>".len());
    let start = std::time::Instant::now();
    for unit in ["[", "*", "`", "**", "[a](", "![", "](", "- ", "  - "] {
        let out = html(&unit.repeat(MAX_MARKDOWN / unit.len()));
        inert(&out);
    }
    let deep: String = (0..200)
        .map(|d| format!("{}- x\n", " ".repeat(d * 2)))
        .collect();
    inert(&html(&deep));
    let nested = "**[*".repeat(2_000);
    inert(&html(&nested));
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    let multibyte = "ğ".repeat(MAX_MARKDOWN);
    assert!(Markdown::new(multibyte).html().len() <= MAX_MARKDOWN + 7);
}
