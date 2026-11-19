//! Checks the body of a screen function before it is compiled.
//!
//! This is not the security boundary; the sandbox is. It keeps the rules of the playground: the
//! code stays inside the screen function, carries no attributes, calls no macros other than
//! zeyn's `t!` and the standard `format!` and `vec!`, and uses no `unsafe`, `extern` or `mod`.
//! Dependencies and build scripts cannot be named at all, because the runner compiles one file
//! with a fixed list of libraries.

/// The longest body accepted, in bytes.
pub const MAX_BODY: usize = 16 * 1024;

/// Macros a body may call.
const MACROS: &[&str] = &["t", "format", "vec"];

/// Words a body may not use.
const WORDS: &[&str] = &["unsafe", "extern", "mod"];

/// Keywords that may stand before a `!` that negates, such as `return !done`.
const BEFORE_NOT: &[&str] = &["return", "if", "while", "match", "in", "else", "break"];

/// Accepts `body` when it follows the rules of the playground, or says what to change.
pub fn body(body: &str) -> Result<(), String> {
    if body.len() > MAX_BODY {
        return Err(format!("the code is longer than {MAX_BODY} bytes"));
    }
    let s: Vec<char> = body.chars().collect();
    let mut open = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match c {
            '/' if s.get(i + 1) == Some(&'/') => {
                while i < s.len() && s[i] != '\n' {
                    i += 1;
                }
            }
            '/' if s.get(i + 1) == Some(&'*') => i = block_comment(&s, i)?,
            '"' => i = string(&s, i + 1)?,
            '\'' => i = quote(&s, i)?,
            '#' => return Err("attributes (#[...]) are not allowed".into()),
            '{' | '(' | '[' => {
                open.push(c);
                i += 1;
            }
            '}' | ')' | ']' => {
                let want = match c {
                    '}' => '{',
                    ')' => '(',
                    _ => '[',
                };
                if open.pop() != Some(want) {
                    return Err(format!(
                        "unmatched `{c}`: the code must stay inside the screen function"
                    ));
                }
                i += 1;
            }
            c if c.is_alphanumeric() || c == '_' => i = word(&s, i)?,
            _ => i += 1,
        }
    }
    match open.last() {
        Some(c) => Err(format!(
            "unclosed `{c}`: the code must stay inside the screen function"
        )),
        None => Ok(()),
    }
}

/// Skips a block comment that starts at `i`, nested ones included.
fn block_comment(s: &[char], mut i: usize) -> Result<usize, String> {
    let mut depth = 0;
    while i + 1 < s.len() {
        if s[i] == '/' && s[i + 1] == '*' {
            depth += 1;
            i += 2;
        } else if s[i] == '*' && s[i + 1] == '/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Ok(i);
            }
        } else {
            i += 1;
        }
    }
    Err("unclosed comment".into())
}

/// Skips a string whose text starts at `i`, up to and with its closing quote.
fn string(s: &[char], mut i: usize) -> Result<usize, String> {
    while i < s.len() {
        match s[i] {
            '\\' => i += 2,
            '"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err("unclosed string".into())
}

/// Skips a raw string whose `#` marks start at `i`.
fn raw_string(s: &[char], mut i: usize) -> Result<usize, String> {
    let mut hashes = 0;
    while s.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    if s.get(i) != Some(&'"') {
        return Err("attributes (#[...]) are not allowed".into());
    }
    i += 1;
    while i < s.len() {
        if s[i] == '"' && (1..=hashes).all(|k| s.get(i + k) == Some(&'#')) {
            return Ok(i + 1 + hashes);
        }
        i += 1;
    }
    Err("unclosed string".into())
}

/// Skips a character literal or a lifetime that starts at `i`.
fn quote(s: &[char], i: usize) -> Result<usize, String> {
    if s.get(i + 1) == Some(&'\\') {
        let mut j = i + 2;
        while j < s.len() && s[j] != '\'' {
            j += 1;
        }
        return match j < s.len() {
            true => Ok(j + 1),
            false => Err("unclosed character".into()),
        };
    }
    if s.get(i + 2) == Some(&'\'') {
        return Ok(i + 3);
    }
    let mut j = i + 1;
    while j < s.len() && (s[j].is_alphanumeric() || s[j] == '_') {
        j += 1;
    }
    Ok(j.max(i + 1))
}

/// Reads a word that starts at `i`: a literal prefix, a keyword, an identifier or a macro name.
fn word(s: &[char], i: usize) -> Result<usize, String> {
    let mut j = i;
    while j < s.len() && (s[j].is_alphanumeric() || s[j] == '_') {
        j += 1;
    }
    let w: String = s[i..j].iter().collect();
    let next = s.get(j);
    match (w.as_str(), next) {
        ("r" | "br" | "cr", Some('"')) => return raw_string(s, j),
        ("r" | "br" | "cr", Some('#'))
            if s.get(j + 1)
                .is_some_and(|c| *c == '#' || *c == '"') =>
        {
            return raw_string(s, j);
        }
        ("r", Some('#')) => return word(s, j + 1),
        ("b" | "c", Some('"')) => return string(s, j + 1),
        ("b", Some('\'')) => return quote(s, j),
        _ => {}
    }
    if WORDS.contains(&w.as_str()) {
        return Err(format!("`{w}` is not allowed"));
    }
    let mut k = j;
    while k < s.len() && s[k].is_whitespace() {
        k += 1;
    }
    let bang = s.get(k) == Some(&'!') && s.get(k + 1) != Some(&'=');
    if bang && !BEFORE_NOT.contains(&w.as_str()) && !MACROS.contains(&w.as_str()) {
        return Err(format!(
            "the macro `{w}!` is not allowed; use t!, format! or vec!"
        ));
    }
    Ok(j)
}

#[cfg(test)]
mod tests {
    use super::body;

    #[test]
    fn a_screen_with_texts_strings_and_comments_passes() {
        let ok = r##"
            // a comment with } and #[attr]
            /* nested /* } */ comment */
            let title = format!("Hello {}", "}");
            let raw = r#"a "}" b"#;
            let c = '}';
            let e = '\'';
            let b = b'#';
            fn pick<'a>(x: &'a str) -> &'a str { x }
            if title != raw { return Screen::new(t!("demo.title")); }
            let not = !vec![1].is_empty();
            Screen::new(pick(&title)).body(Paragraph::new(c.to_string() + &e.to_string() + &(b as char).to_string() + &not.to_string()))
        "##;
        assert_eq!(body(ok), Ok(()));
    }

    #[test]
    fn code_that_leaves_the_function_is_rejected() {
        let escape = "Screen::new(\"a\") } fn main() { loop {} } fn x() -> Screen {";
        assert!(
            body(escape)
                .unwrap_err()
                .contains("must stay inside")
        );
        assert!(body("Screen::new(\"a\")").is_ok());
        assert!(body("{ Screen::new(\"a\")").is_err());
        assert!(body("(]").is_err());
    }

    #[test]
    fn attributes_macros_and_unsafe_are_rejected() {
        for bad in [
            "#![allow(warnings)] Screen::new(\"a\")",
            "#[path = \"/etc\"] mod x;",
            "include_str!(\"/etc/passwd\"); Screen::new(\"a\")",
            "std::include_bytes ! (\"/etc/passwd\"); Screen::new(\"a\")",
            "macro_rules! m { () => {} } Screen::new(\"a\")",
            "std::arch::asm!(\"nop\"); Screen::new(\"a\")",
            "unsafe { } Screen::new(\"a\")",
            "extern \"C\" { fn f(); } Screen::new(\"a\")",
            "r#unsafe!(); Screen::new(\"a\")",
        ] {
            assert!(body(bad).is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn unclosed_literals_and_long_code_are_rejected() {
        assert!(body("\"open").is_err());
        assert!(body("r#\"open\"").is_err());
        assert!(body("/* open").is_err());
        assert!(body(&"a".repeat(super::MAX_BODY + 1)).is_err());
    }
}
