//! The content security policy of every page and the host allowlists that components read.
//!
//! The policy is strict by default: everything comes from the app's own origin. An app opens it
//! per directive with [`Csp`] (`App::csp`), and only for named `https` hosts. `script-src`,
//! `object-src`, `base-uri` and `frame-ancestors` never change. The header text is built once
//! when the router is built.
//!
//! Components that point at outside hosts ([`crate::ui::Embed`], [`crate::ui::Image`]) read the
//! same lists from the request context, so a host the browser would block is never rendered.

/// Longest URL that [`embed_url`] keeps.
pub const MAX_EMBED_URL: usize = 2048;

/// Query parameters that only track the visitor. [`embed_url`] removes them, and every
/// parameter whose name starts with `utm_`.
pub const TRACKING_PARAMS: [&str; 10] =
    ["fbclid", "gclid", "gclsrc", "dclid", "msclkid", "yclid", "igshid", "mc_cid", "mc_eid", "_ga"];

/// One allowed host of a directive: `https://host`, `https://host:port` or `https://*.host`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    host: String,
    wildcard: bool,
    port: Option<u16>,
}

impl Source {
    fn matches(&self, host: &str, port: Option<u16>) -> bool {
        if self.port != port {
            return false;
        }
        if self.wildcard {
            host.len() > self.host.len() + 1
                && host.ends_with(self.host.as_str())
                && host.as_bytes()[host.len() - self.host.len() - 1] == b'.'
        } else {
            host == self.host
        }
    }

    fn write(&self, out: &mut String) {
        out.push_str(" https://");
        if self.wildcard {
            out.push_str("*.");
        }
        out.push_str(&self.host);
        if let Some(p) = self.port {
            out.push(':');
            out.push_str(&p.to_string());
        }
    }
}

/// Extra hosts for the page's content security policy. Built with [`crate::App::csp`]:
///
/// ```
/// # use zeyn::App;
/// let app = App::new("Shop").csp(|c| {
///     c.img_src(["https://images.example.com"])
///         .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
///         .media_src(["https://media.example.com:8443"])
///         .media_blob()
/// });
/// assert!(app.check().is_ok());
/// ```
///
/// Only `https://host`, `https://host:port` and `https://*.host` are accepted. `*`, `http:`,
/// bare schemes such as `data:`, quoted keywords such as `'unsafe-inline'`, paths and anything
/// with spaces or semicolons are refused, and `App::check` reports them:
///
/// ```
/// # use zeyn::App;
/// let app = App::new("Shop").csp(|c| c.img_src(["*", "http://cdn.example.com", "'unsafe-inline'"]));
/// assert_eq!(app.check().unwrap_err().len(), 3);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Csp {
    img: Vec<Source>,
    media: Vec<Source>,
    frame: Vec<Source>,
    connect: Vec<Source>,
    font: Vec<Source>,
    media_blob: bool,
    problems: Vec<String>,
}

/// The empty policy, for contexts built outside a request.
pub(crate) static EMPTY: Csp = Csp::new();

impl Csp {
    /// The default policy: nothing beyond the app's own origin.
    pub const fn new() -> Self {
        Csp {
            img: Vec::new(),
            media: Vec::new(),
            frame: Vec::new(),
            connect: Vec::new(),
            font: Vec::new(),
            media_blob: false,
            problems: Vec::new(),
        }
    }

    fn add<'a>(mut self, directive: &'static str, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        for value in hosts {
            match parse_source(value) {
                Ok(s) => {
                    let list = match directive {
                        "img-src" => &mut self.img,
                        "media-src" => &mut self.media,
                        "frame-src" => &mut self.frame,
                        "connect-src" => &mut self.connect,
                        _ => &mut self.font,
                    };
                    if !list.contains(&s) {
                        list.push(s);
                    }
                }
                Err(help) => {
                    self.problems.push(format!("csp {directive}: `{value}` is not accepted\n  = help: {help}"))
                }
            }
        }
        self
    }

    /// Hosts images may come from, besides the app itself and `data:` and `blob:` URLs.
    pub fn img_src<'a>(self, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        self.add("img-src", hosts)
    }

    /// Hosts audio and video may come from, besides the app itself.
    pub fn media_src<'a>(self, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        self.add("media-src", hosts)
    }

    /// Lets audio and video play from `blob:` URLs, which streaming libraries use.
    pub fn media_blob(mut self) -> Self {
        self.media_blob = true;
        self
    }

    /// Hosts pages may frame with [`crate::ui::Embed`], besides the app itself.
    pub fn frame_src<'a>(self, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        self.add("frame-src", hosts)
    }

    /// Hosts browser code may call with `fetch`, WebSocket or `EventSource`, besides the app itself.
    pub fn connect_src<'a>(self, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        self.add("connect-src", hosts)
    }

    /// Hosts fonts may come from, besides the app itself.
    pub fn font_src<'a>(self, hosts: impl IntoIterator<Item = &'a str>) -> Self {
        self.add("font-src", hosts)
    }

    /// What `App::check` reports: every refused value with what to write instead.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// The header text. `wasm` adds `'wasm-unsafe-eval'` to `script-src` for pages that hold a
    /// WASM island. With nothing added it is the strict default policy.
    ///
    /// ```
    /// # use zeyn::Csp;
    /// let h = Csp::new().frame_src(["https://player.example.com"]).header(false);
    /// assert!(h.contains("frame-src 'self' https://player.example.com;"));
    /// assert!(h.contains("script-src 'self';") && h.ends_with("frame-ancestors 'none'"));
    /// ```
    pub fn header(&self, wasm: bool) -> String {
        let mut h = String::with_capacity(320);
        h.push_str("default-src 'self'; script-src 'self'");
        if wasm {
            h.push_str(" 'wasm-unsafe-eval'");
        }
        h.push_str("; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:");
        self.img.iter().for_each(|s| s.write(&mut h));
        h.push_str("; ");
        if !self.media.is_empty() || self.media_blob {
            h.push_str("media-src 'self'");
            if self.media_blob {
                h.push_str(" blob:");
            }
            self.media.iter().for_each(|s| s.write(&mut h));
            h.push_str("; ");
        }
        for (name, list) in [("frame-src", &self.frame), ("connect-src", &self.connect), ("font-src", &self.font)] {
            if !list.is_empty() {
                h.push_str(name);
                h.push_str(" 'self'");
                list.iter().for_each(|s| s.write(&mut h));
                h.push_str("; ");
            }
        }
        h.push_str("object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'");
        h
    }

    /// Whether `url` is an `https` address on a host of `frame-src`, so an
    /// [`crate::ui::Embed`] of it would load.
    pub fn allows_frame(&self, url: &str) -> bool {
        allowed(&self.frame, url)
    }

    /// Whether an image at `src` would load: a path on the app itself (`/logo.png`) or an
    /// `https` address on a host of `img-src`.
    pub fn allows_image(&self, src: &str) -> bool {
        (src.starts_with('/') && !src.starts_with("//") && !src.contains('\\')) || allowed(&self.img, src)
    }
}

fn allowed(list: &[Source], url: &str) -> bool {
    https_authority(url).is_some_and(|(host, port)| list.iter().any(|s| s.matches(&host, port)))
}

/// Reads one allowlist value. The error is the help line of the problem.
fn parse_source(value: &str) -> Result<Source, &'static str> {
    const HOST_ONLY: &str = "write `https://host`, `https://host:port` or `https://*.host`";
    if value.is_empty() || value.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, ';' | ',')) {
        return Err("spaces, commas and semicolons would add other sources; give one host per entry");
    }
    if value.contains('\'') || value.contains('"') {
        return Err("quoted keywords such as 'unsafe-inline' or 'unsafe-eval' are never allowed");
    }
    let lower = value.to_ascii_lowercase();
    if lower == "*" {
        return Err("`*` allows every host; name the hosts you need, such as `https://cdn.example.com`");
    }
    if lower.starts_with("http://") || lower == "http:" {
        return Err("only `https` hosts are accepted; plain `http` can be read and changed on the way");
    }
    let Some(rest) = lower.strip_prefix("https://") else {
        return Err(
            "bare schemes (`https:`, `data:`, `blob:`) and other schemes are refused; name a host, and use `.media_blob()` for `blob:` media",
        );
    };
    if rest.contains(['/', '?', '#', '@', '\\']) {
        return Err("give only the host, without a path, query or user name; write `https://host`");
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(n) if n > 0 && !p.starts_with('0') => (h, Some(n)),
            _ => return Err("the port must be a number from 1 to 65535"),
        },
        None => (rest, None),
    };
    let port = port.filter(|p| *p != 443);
    let (host, wildcard) = match host.strip_prefix("*.") {
        Some(h) => (h, true),
        None => (host, false),
    };
    if !valid_host(host) || (wildcard && !host.contains('.')) {
        return Err(HOST_ONLY);
    }
    Ok(Source { host: host.to_string(), wildcard, port })
}

/// Host name labels of letters, digits and inner hyphens, such as `cdn.example.com`.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Host (lower case) and port (`None` for 443) of an `https` URL, or `None` when it is not one.
fn https_authority(url: &str) -> Option<(String, Option<u16>)> {
    let rest = url.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")).map(|_| &url[8..])?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..end].to_ascii_lowercase();
    if authority.contains(['@', '\\']) {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), Some(p.parse::<u16>().ok().filter(|n| *n > 0)?)),
        None => (authority, None),
    };
    valid_host(&host).then_some((host, port.filter(|p| *p != 443)))
}

/// The canonical form of an address to embed, or `None` when it cannot be embedded.
///
/// Takes a plain URL or pasted embed code (`<iframe ... src="...">`), of which only `src` is
/// kept. The address must be `https` with a plain host; the host is written in lower case, the
/// port 443 is dropped, and tracking parameters (`utm_*` and [`TRACKING_PARAMS`]) are removed.
/// Scripts, `javascript:` and `data:` addresses, user names and quotes give `None`.
///
/// ```
/// # use zeyn::embed_url;
/// assert_eq!(
///     embed_url(r#"<iframe width="560" src="https://WWW.Example.com/embed/x?utm_source=a&amp;t=4" allowfullscreen></iframe>"#),
///     Some("https://www.example.com/embed/x?t=4".to_string())
/// );
/// assert_eq!(embed_url("javascript:alert(1)"), None);
/// ```
pub fn embed_url(input: &str) -> Option<String> {
    let input = input.trim();
    let url = if input.starts_with('<') { iframe_src(input)? } else { input.to_string() };
    canonical(&url)
}

fn iframe_src(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<iframe")?;
    let end = start + lower[start..].find('>')?;
    let tag = &html[start..end];
    let tag_lower = &lower[start..end];
    let mut from = 0;
    let at = loop {
        let i = from + tag_lower[from..].find("src")?;
        let before = tag_lower.as_bytes()[i - 1];
        let after = tag_lower[i + 3..].trim_start();
        if before.is_ascii_whitespace() && after.starts_with('=') {
            break i + 3 + (tag_lower[i + 3..].len() - after.len()) + 1;
        }
        from = i + 3;
    };
    let value = tag[at..].trim_start();
    let raw = match value.chars().next()? {
        q @ ('"' | '\'') => &value[1..1 + value[1..].find(q)?],
        _ => value.split(|c: char| c.is_whitespace() || c == '>').next()?,
    };
    Some(raw.replace("&amp;", "&").replace("&#38;", "&"))
}

fn canonical(url: &str) -> Option<String> {
    if url.len() > MAX_EMBED_URL
        || url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '\\'))
    {
        return None;
    }
    let (host, port) = https_authority(url)?;
    let rest = &url[8..];
    let tail = &rest[rest.find(['/', '?', '#']).unwrap_or(rest.len())..];
    let (before_fragment, fragment) = match tail.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (tail, None),
    };
    let (path, query) = match before_fragment.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (before_fragment, None),
    };
    let mut out = String::with_capacity(url.len());
    out.push_str("https://");
    out.push_str(&host);
    if let Some(p) = port {
        out.push(':');
        out.push_str(&p.to_string());
    }
    out.push_str(if path.is_empty() { "/" } else { path });
    let kept: Vec<&str> = query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter(|pair| {
            let name = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
            !pair.is_empty() && !name.starts_with("utm_") && !TRACKING_PARAMS.contains(&name.as_str())
        })
        .collect();
    if !kept.is_empty() {
        out.push('?');
        out.push_str(&kept.join("&"));
    }
    if let Some(f) = fragment.filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(f);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_header_is_the_strict_policy() {
        assert_eq!(
            Csp::new().header(false),
            "default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'"
        );
    }

    #[test]
    fn wildcards_match_subdomains_only() {
        let c = Csp::new().frame_src(["https://*.example.com", "https://player.test:8443"]);
        assert!(c.allows_frame("https://a.example.com/x"));
        assert!(!c.allows_frame("https://example.com/x"));
        assert!(!c.allows_frame("https://badexample.com/x"));
        assert!(c.allows_frame("https://player.test:8443/"));
        assert!(!c.allows_frame("https://player.test/"));
        assert!(!c.allows_frame("http://a.example.com/"));
        assert!(!c.allows_frame("https://user@a.example.com/"));
    }

    #[test]
    fn iframe_src_reads_the_src_attribute_only() {
        assert_eq!(
            embed_url(r#"<iframe data-src="https://evil.test/" src='https://ok.test/a'></iframe>"#).as_deref(),
            Some("https://ok.test/a")
        );
        assert_eq!(embed_url("<script>alert(1)</script>"), None);
        assert_eq!(embed_url("https://ok.test:443?gclid=1#t=3").as_deref(), Some("https://ok.test/#t=3"));
    }
}
