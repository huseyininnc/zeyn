//! Syntax highlighting on the server for Rust, TOML, JSON, shell and JavaScript.
//!
//! The output is escaped text with `<span class="z-hl-*">` around keywords, strings, numbers,
//! comments, types, macros and keys. The colors come from theme tokens in `zeyn.css`, so they
//! follow the light and dark themes and cost nothing on the client. Other languages are only
//! escaped.

use zeyn::render::escape;

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const JS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

const JSON: &[&str] = &["true", "false", "null"];

const TOML: &[&str] = &["true", "false"];

const SHELL: &[&str] = &[
    "if", "then", "else", "fi", "for", "do", "done", "export", "cd", "echo",
];

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Rust,
    Js,
    Json,
    Toml,
    Shell,
}

impl Lang {
    fn of(name: &str) -> Option<Lang> {
        Some(match name.to_ascii_lowercase().as_str() {
            "rust" | "rs" => Lang::Rust,
            "js" | "javascript" | "mjs" => Lang::Js,
            "json" => Lang::Json,
            "toml" => Lang::Toml,
            "sh" | "shell" | "bash" | "console" => Lang::Shell,
            _ => return None,
        })
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Rust => RUST,
            Lang::Js => JS,
            Lang::Json => JSON,
            Lang::Toml => TOML,
            Lang::Shell => SHELL,
        }
    }

    fn line_comment(self) -> Option<&'static str> {
        match self {
            Lang::Rust | Lang::Js => Some("//"),
            Lang::Toml | Lang::Shell => Some("#"),
            Lang::Json => None,
        }
    }
}

/// Escaped, highlighted HTML for a code block in `lang`.
pub fn highlight(lang: &str, code: &str) -> String {
    match Lang::of(lang) {
        Some(l) => Tokens {
            lang: l,
            chars: code.chars().collect(),
            out: String::with_capacity(code.len() * 2),
        }
        .run(),
        None => escape(code),
    }
}

struct Tokens {
    lang: Lang,
    chars: Vec<char>,
    out: String,
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Tokens {
    fn span(&mut self, class: &str, from: usize, to: usize) {
        let text: String = self.chars[from..to].iter().collect();
        self.out.push_str(&format!(
            r#"<span class="z-hl-{class}">{}</span>"#,
            escape(&text)
        ));
    }

    fn plain(&mut self, c: char) {
        self.out
            .push_str(&escape(&c.to_string()));
    }

    fn at(&self, i: usize, pat: &str) -> bool {
        pat.chars()
            .enumerate()
            .all(|(k, p)| self.chars.get(i + k) == Some(&p))
    }

    fn line_start(&self, i: usize) -> bool {
        self.chars[..i]
            .iter()
            .rev()
            .take_while(|c| **c != '\n')
            .all(|c| c.is_whitespace())
    }

    /// End of a string that starts at `i` with `quote`, after the closing quote.
    fn string_end(&self, i: usize, quote: char) -> usize {
        let mut k = i + 1;
        while k < self.chars.len() {
            match self.chars[k] {
                '\\' if self.lang != Lang::Toml || quote == '"' => k += 2,
                c if c == quote => return k + 1,
                '\n' if self.lang != Lang::Rust => return k,
                _ => k += 1,
            }
        }
        self.chars.len()
    }

    fn run(mut self) -> String {
        let mut i = 0;
        let n = self.chars.len();
        while i < n {
            let c = self.chars[i];
            if let Some(lc) = self.lang.line_comment() {
                if self.at(i, lc) && (lc != "#" || i == 0 || self.chars[i - 1].is_whitespace()) {
                    let end = (i..n)
                        .find(|&k| self.chars[k] == '\n')
                        .unwrap_or(n);
                    self.span("com", i, end);
                    i = end;
                    continue;
                }
            }
            if matches!(self.lang, Lang::Rust | Lang::Js) && self.at(i, "/*") {
                let end = (i + 2..n)
                    .find(|&k| self.at(k, "*/"))
                    .map(|k| k + 2)
                    .unwrap_or(n);
                self.span("com", i, end);
                i = end;
                continue;
            }
            if self.lang == Lang::Rust
                && c == '#'
                && matches!(self.chars.get(i + 1), Some('[') | Some('!'))
            {
                let mut depth = 0;
                let mut end = i + 1;
                while end < n {
                    match self.chars[end] {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                end += 1;
                                break;
                            }
                        }
                        '\n' => break,
                        _ => {}
                    }
                    end += 1;
                }
                self.span("mac", i, end);
                i = end;
                continue;
            }
            if self.lang == Lang::Rust
                && c == 'r'
                && (self.at(i, "r\"") || self.at(i, "r#\""))
                && (i == 0 || !ident_char(self.chars[i - 1]))
            {
                let hashes = self.chars[i + 1..]
                    .iter()
                    .take_while(|c| **c == '#')
                    .count();
                let close: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let start = i + 2 + hashes;
                let end = (start..n)
                    .find(|&k| self.at(k, &close))
                    .map(|k| k + close.len())
                    .unwrap_or(n);
                self.span("str", i, end);
                i = end;
                continue;
            }
            if c == '"'
                || (c == '\'' && matches!(self.lang, Lang::Js | Lang::Shell | Lang::Toml))
                || (c == '`' && self.lang == Lang::Js)
            {
                let end = self.string_end(i, c);
                let next = self.chars[end..]
                    .iter()
                    .find(|c| !c.is_whitespace());
                let key = (self.lang == Lang::Json && next == Some(&':'))
                    || (self.lang == Lang::Toml && self.line_start(i) && next == Some(&'='));
                self.span(if key { "key" } else { "str" }, i, end);
                i = end;
                continue;
            }
            if c == '\'' && self.lang == Lang::Rust {
                let close = if self.chars.get(i + 1) == Some(&'\\') {
                    (i + 2..(i + 12).min(n)).find(|&k| self.chars[k] == '\'')
                } else {
                    (self.chars.get(i + 2) == Some(&'\'')).then_some(i + 2)
                };
                if let Some(end) = close {
                    self.span("str", i, end + 1);
                    i = end + 1;
                    continue;
                }
            }
            if c.is_ascii_digit() && (i == 0 || !ident_char(self.chars[i - 1])) {
                let end = (i..n)
                    .find(|&k| {
                        !(ident_char(self.chars[k])
                            || (self.chars[k] == '.'
                                && self
                                    .chars
                                    .get(k + 1)
                                    .is_some_and(char::is_ascii_digit)))
                    })
                    .unwrap_or(n);
                self.span("num", i, end);
                i = end;
                continue;
            }
            if self.lang == Lang::Toml && c == '[' && self.line_start(i) {
                let end = (i..n)
                    .find(|&k| self.chars[k] == '\n')
                    .unwrap_or(n);
                self.span("kw", i, end);
                i = end;
                continue;
            }
            if self.lang == Lang::Shell && c == '-' && (i == 0 || self.chars[i - 1].is_whitespace())
            {
                let end = (i..n)
                    .find(|&k| self.chars[k].is_whitespace() || self.chars[k] == '=')
                    .unwrap_or(n);
                self.span("ty", i, end);
                i = end;
                continue;
            }
            if ident_char(c) {
                let end = (i..n)
                    .find(|&k| {
                        !(ident_char(self.chars[k])
                            || (self.lang == Lang::Shell && matches!(self.chars[k], '-' | '.')))
                    })
                    .unwrap_or(n);
                let word: String = self.chars[i..end].iter().collect();
                let next = self.chars[end..]
                    .iter()
                    .find(|c| !c.is_whitespace() || **c == '\n')
                    .copied();
                let class = if self
                    .lang
                    .keywords()
                    .contains(&word.as_str())
                {
                    Some("kw")
                } else if self.lang == Lang::Rust && self.chars.get(end) == Some(&'!') {
                    self.span("mac", i, end + 1);
                    i = end + 1;
                    continue;
                } else if self.lang == Lang::Toml && self.line_start(i) && next == Some('=') {
                    Some("key")
                } else if self.lang == Lang::Shell && self.line_start(i) {
                    Some("kw")
                } else if matches!(self.lang, Lang::Rust | Lang::Js) && c.is_uppercase() {
                    Some("ty")
                } else {
                    None
                };
                match class {
                    Some(k) => self.span(k, i, end),
                    None => self.out.push_str(&escape(&word)),
                }
                i = end;
                continue;
            }
            self.plain(c);
            i += 1;
        }
        self.out
    }
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
