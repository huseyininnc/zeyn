//! Syntax highlighting of code blocks, done by the framework's highlighter
//! ([`zeyn::ui::highlight`]) on the server.
//!
//! The output is escaped text with `<span class="z-hl-*">` around keywords, strings, numbers,
//! comments, types, macros and keys. The colors come from theme tokens in `zeyn.css`, so they
//! follow the light and dark themes and cost nothing on the client. Languages the framework does
//! not know are only escaped.

use zeyn::ui::CodeLang;

/// Escaped, highlighted HTML for a code block in `lang`, a fence name such as `rust` or `sh`.
pub fn highlight(lang: &str, code: &str) -> String {
    zeyn::ui::highlight(CodeLang::named(lang), code)
}

#[cfg(test)]
mod tests {
    use super::highlight;

    #[test]
    fn rust_keywords_types_strings_macros_and_comments() {
        let h = highlight(
            "rust",
            "#[screen(route = \"/x\")]\npub fn x() -> Screen { t!(\"a\") } // done\nlet n = 42;",
        );
        assert!(
            h.starts_with(r#"<span class="z-hl-mac">#[screen(route = &quot;/x&quot;)]</span>"#),
            "{h}"
        );
        assert!(h.contains(r#"<span class="z-hl-kw">pub</span> <span class="z-hl-kw">fn</span>"#));
        assert!(h.contains(r#"<span class="z-hl-ty">Screen</span>"#));
        assert!(h.contains(
            r#"<span class="z-hl-mac">t!</span>(<span class="z-hl-str">&quot;a&quot;</span>)"#
        ));
        assert!(h.contains(r#"<span class="z-hl-com">// done</span>"#));
        assert!(h.contains(r#"<span class="z-hl-num">42</span>"#));
    }

    #[test]
    fn json_keys_toml_tables_and_shell_commands() {
        let j = highlight("json", r#"{ "a": "b", "n": 1, "ok": true }"#);
        assert!(
            j.contains(r#"<span class="z-hl-key">&quot;a&quot;</span>: <span class="z-hl-str">&quot;b&quot;</span>"#),
            "{j}"
        );
        assert!(j.contains(r#"<span class="z-hl-kw">true</span>"#));
        let t = highlight("toml", "[dependencies]\nzeyn = { path = \"x\" } # local");
        assert!(
            t.starts_with(r#"<span class="z-hl-kw">[dependencies]</span>"#),
            "{t}"
        );
        assert!(t.contains(r#"<span class="z-hl-key">zeyn</span>"#));
        assert!(t.contains(r#"<span class="z-hl-com"># local</span>"#));
        let s = highlight("sh", "cargo run -p zeyn-cli -- new app");
        assert!(
            s.starts_with(
                r#"<span class="z-hl-kw">cargo</span> run <span class="z-hl-ty">-p</span>"#
            ),
            "{s}"
        );
    }

    #[test]
    fn everything_is_escaped_and_unknown_languages_stay_plain() {
        let js = highlight("js", "const s = '<script>alert(1)</script>';");
        assert!(!js.contains("<script"));
        assert!(js.contains("&lt;script&gt;"));
        assert_eq!(highlight("html", "<b>x</b>"), "&lt;b&gt;x&lt;/b&gt;");
    }
}
