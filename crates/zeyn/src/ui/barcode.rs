//! [`Barcode`] (Code 128) and [`QrCode`]: drawn as SVG on the server, written by hand after the
//! public standards (ISO/IEC 15417 for Code 128, ISO/IEC 18004 for QR codes), with their quiet
//! zones and a text label for screen readers and for people.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};
use std::fmt::Write as _;

into_node!(Barcode, QrCode);

/// Most characters one [`Barcode`] encodes.
pub const MAX_BARCODE_CHARS: usize = 80;

/// Most bytes one [`QrCode`] encodes: version 40 at the lowest error correction level.
pub const MAX_QR_BYTES: usize = 2953;

/// Bar and space widths of the Code 128 symbols 0 to 106, in modules. 103 to 105 start code
/// sets A, B and C; 106 is the stop pattern with its final bar.
const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

const START_B: u8 = 104;
const START_C: u8 = 105;
const TO_B: u8 = 100;
const TO_C: u8 = 99;
const STOP: u8 = 106;

/// Quiet zone of a Code 128 symbol on each side, in modules (at least 10 by the standard).
pub const CODE128_QUIET: u32 = 10;

/// Quiet zone of a QR code on each side, in modules (4 by the standard).
pub const QR_QUIET: u32 = 4;

/// Symbol values of `text` in Code 128 with the check symbol and the stop, or `None` when a
/// character is not printable ASCII (32 to 126) or the text is empty or too long. Runs of
/// digits use code set C (two digits a symbol) where that is shorter; the rest uses code set B.
///
/// ```
/// use zeyn::ui::code128_symbols;
/// assert_eq!(code128_symbols("PJJ123C"), Some(vec![104, 48, 42, 42, 17, 18, 19, 35, 55, 106]));
/// assert_eq!(code128_symbols("123456"), Some(vec![105, 12, 34, 56, 44, 106]));
/// assert_eq!(code128_symbols("ü"), None);
/// ```
pub fn code128_symbols(text: &str) -> Option<Vec<u8>> {
    let b = text.as_bytes();
    if b.is_empty() || b.len() > MAX_BARCODE_CHARS || !b.iter().all(|c| (32..=126).contains(c)) {
        return None;
    }
    let digits = |from: usize| {
        b[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
    };
    let mut out = Vec::with_capacity(b.len() + 4);
    let mut in_c = false;
    let mut i = 0;
    while i < b.len() {
        let run = digits(i);
        let to_end = i + run == b.len();
        let worth = if i == 0 {
            run >= 4 || (run == 2 && to_end)
        } else {
            run >= 6 || (run >= 4 && to_end)
        };
        if in_c || worth {
            let pairs = run / 2;
            if pairs > 0 {
                if !in_c {
                    out.push(if out.is_empty() { START_C } else { TO_C });
                    in_c = true;
                }
                for p in 0..pairs {
                    let at = i + p * 2;
                    out.push((b[at] - b'0') * 10 + (b[at + 1] - b'0'));
                }
                i += pairs * 2;
                continue;
            }
        }
        if out.is_empty() {
            out.push(START_B);
        } else if in_c {
            out.push(TO_B);
            in_c = false;
        }
        out.push(b[i] - 32);
        i += 1;
    }
    let check = out
        .iter()
        .enumerate()
        .map(|(k, v)| u32::from(*v) * (k.max(1) as u32))
        .sum::<u32>()
        % 103;
    out.push(check as u8);
    out.push(STOP);
    Some(out)
}

/// The modules of a Code 128 symbol, `true` for a bar, without the quiet zones.
fn code128_modules(symbols: &[u8]) -> Vec<bool> {
    let mut out = Vec::with_capacity(symbols.len() * 11 + 2);
    for s in symbols {
        for (k, w) in CODE128[usize::from(*s)]
            .bytes()
            .enumerate()
        {
            out.extend(std::iter::repeat_n(k % 2 == 0, usize::from(w - b'0')));
        }
    }
    out
}

/// A label for a figure: the given one, or the encoded text.
fn label_of(label: &Option<Text>, value: &str, r: &Render) -> String {
    match label {
        Some(t) => r.text(t),
        None => escape(value),
    }
}

/// A figure with the symbol, its caption and the screen reader name.
fn figure(out: &mut String, r: &Render, class: &str, svg: &str, caption: &str, kind: &str) {
    let id = super::dom_id(r, kind);
    out.push_str(&format!(
        r#"<figure class="z-barcode {class}" role="img" aria-labelledby="{id}"><div class="z-barcode-plate" aria-hidden="true">{svg}</div><figcaption class="z-barcode-text" id="{id}">{caption}</figcaption></figure>"#
    ));
}

/// What a figure shows when the value cannot be encoded.
fn refused(out: &mut String, r: &Render, key: &str) {
    out.push_str(&format!(
        r#"<p class="z-barcode z-barcode-error" role="note">{}</p>"#,
        r.text(&Text::key(format!("zeyn.ui.barcode.{key}")))
    ));
}

/// A Code 128 barcode, drawn as SVG on the server with its quiet zones and the text under it.
/// It encodes printable ASCII, at most [`MAX_BARCODE_CHARS`] characters; digit runs are packed
/// two to a symbol. A value it cannot encode shows a short notice instead.
///
/// The symbol is black on a white plate in both themes, as scanners need. Screen readers hear
/// the figure as an image named by the text under it ([`Barcode::label`] when set).
///
/// ```
/// use zeyn::ui::Barcode;
/// use zeyn::{Component, I18n, Render};
///
/// let mut html = String::new();
/// Barcode::code128("ORD-2026-0042").render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains("<svg") && html.contains(">ORD-2026-0042</figcaption>"));
/// ```
pub struct Barcode {
    value: String,
    label: Option<Text>,
    height: u32,
}

impl Barcode {
    /// A Code 128 barcode of `value`.
    pub fn code128(value: impl Into<String>) -> Self {
        Barcode {
            value: value.into(),
            label: None,
            height: 64,
        }
    }

    /// The text under the bars and the name screen readers hear, instead of the value.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Height of the bars in CSS pixels, 24 to 400; 64 unless set. The width follows the
    /// number of modules, two pixels each, and shrinks with a narrow container.
    pub fn height(mut self, px: u32) -> Self {
        self.height = px.clamp(24, 400);
        self
    }

    /// The SVG of the symbol, or `None` when the value cannot be encoded.
    pub fn svg(&self) -> Option<String> {
        let modules = code128_modules(&code128_symbols(&self.value)?);
        let width = modules.len() as u32 + 2 * CODE128_QUIET;
        let mut path = String::with_capacity(modules.len() * 4);
        let mut x = 0;
        while x < modules.len() {
            if !modules[x] {
                x += 1;
                continue;
            }
            let run = modules[x..]
                .iter()
                .take_while(|m| **m)
                .count();
            let _ = write!(path, "M{} 0h{run}v1h-{run}z", x as u32 + CODE128_QUIET);
            x += run;
        }
        Some(format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 1" width="{}" height="{}" preserveAspectRatio="none" shape-rendering="crispEdges" focusable="false"><rect width="{width}" height="1" fill="#fff"/><path fill="#000" d="{path}"/></svg>"##,
            width * 2,
            self.height
        ))
    }
}

impl Component for Barcode {
    fn render(&self, out: &mut String, r: &Render) {
        match self.svg() {
            Some(svg) => figure(
                out,
                r,
                "z-barcode-linear",
                &svg,
                &label_of(&self.label, &self.value, r),
                "barcode",
            ),
            None => refused(out, r, "code128_invalid"),
        }
    }

    fn schema(&self) -> Json {
        json!({ "type": "barcode", "symbology": "code128", "value": self.value, "label": self.label })
    }
}

/// How much of a [`QrCode`] can be damaged and still read: about 7, 15, 25 or 30 percent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QrLevel {
    Low,
    #[default]
    Medium,
    Quartile,
    High,
}

impl QrLevel {
    fn index(self) -> usize {
        self as usize
    }

    /// The two format bits of the level.
    fn bits(self) -> u32 {
        match self {
            QrLevel::Low => 1,
            QrLevel::Medium => 0,
            QrLevel::Quartile => 3,
            QrLevel::High => 2,
        }
    }
}

/// Error correction codewords per block, by level and version (index 0 unused).
const ECC_PER_BLOCK: [[u8; 41]; 4] = [
    [
        0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28,
        30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28,
        28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    ],
    [
        0, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30,
        30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24,
        30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
];

/// Error correction blocks, by level and version (index 0 unused).
const BLOCKS: [[u8; 41]; 4] = [
    [
        0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13,
        14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
    ],
    [
        0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21,
        23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
    ],
    [
        0, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29,
        34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68,
    ],
    [
        0, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32,
        35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81,
    ],
];

/// Modules of a version that hold data and error correction, after the function patterns.
fn raw_modules(version: usize) -> usize {
    let v = version;
    let mut n = (16 * v + 128) * v + 64;
    if v >= 2 {
        let align = v / 7 + 2;
        n -= (25 * align - 10) * align - 55;
        if v >= 7 {
            n -= 36;
        }
    }
    n
}

/// Data codewords of a version at a level.
fn data_codewords(version: usize, level: QrLevel) -> usize {
    let l = level.index();
    raw_modules(version) / 8
        - usize::from(ECC_PER_BLOCK[l][version]) * usize::from(BLOCKS[l][version])
}

/// Product in GF(2^8) with the QR polynomial `x^8 + x^4 + x^3 + x^2 + 1`.
fn gf_mul(x: u8, y: u8) -> u8 {
    let mut z: u16 = 0;
    for i in (0..8).rev() {
        z = (z << 1) ^ ((z >> 7) * 0x11D);
        z ^= u16::from((y >> i) & 1) * u16::from(x);
    }
    z as u8
}

/// The Reed Solomon generator polynomial of a degree, highest term left out.
fn rs_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0u8; degree];
    result[degree - 1] = 1;
    let mut root = 1u8;
    for _ in 0..degree {
        for j in 0..degree {
            result[j] = gf_mul(result[j], root);
            if j + 1 < degree {
                result[j] ^= result[j + 1];
            }
        }
        root = gf_mul(root, 2);
    }
    result
}

/// Reed Solomon error correction codewords of `data` for a divisor.
///
/// ```
/// use zeyn::ui::qr_error_correction;
/// let data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
/// assert_eq!(qr_error_correction(&data, 10), [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
/// ```
pub fn qr_error_correction(data: &[u8], degree: usize) -> Vec<u8> {
    let divisor = rs_divisor(degree);
    let mut result = vec![0u8; degree];
    for b in data {
        let factor = b ^ result[0];
        result.rotate_left(1);
        result[degree - 1] = 0;
        for (r, d) in result.iter_mut().zip(&divisor) {
            *r ^= gf_mul(*d, factor);
        }
    }
    result
}

/// The 15 format bits of a level and mask, with their BCH code and the standard's XOR mask.
///
/// ```
/// use zeyn::ui::{QrLevel, qr_format_bits};
/// assert_eq!(qr_format_bits(QrLevel::Low, 0), 0b111011111000100);
/// assert_eq!(qr_format_bits(QrLevel::Medium, 0), 0b101010000010010);
/// assert_eq!(qr_format_bits(QrLevel::High, 7), 0b000100000111011);
/// ```
pub fn qr_format_bits(level: QrLevel, mask: u8) -> u32 {
    let data = level.bits() << 3 | u32::from(mask);
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    (data << 10 | rem) ^ 0x5412
}

/// The 18 version bits of versions 7 and up.
///
/// ```
/// assert_eq!(zeyn::ui::qr_version_bits(7), 0x07C94);
/// ```
pub fn qr_version_bits(version: usize) -> u32 {
    let v = version as u32;
    let mut rem = v;
    for _ in 0..12 {
        rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
    }
    v << 12 | rem
}

/// A QR code symbol: its modules, row by row.
pub struct QrMatrix {
    size: usize,
    dark: Vec<bool>,
    function: Vec<bool>,
    version: usize,
    mask: u8,
}

impl QrMatrix {
    /// The symbol of `data` in byte mode at the smallest version that holds it at `level`, or
    /// `None` when it is longer than [`MAX_QR_BYTES`] (or than the level allows).
    pub fn encode(data: &[u8], level: QrLevel) -> Option<QrMatrix> {
        let version = (1..=40).find(|v| {
            let count = if *v <= 9 { 8 } else { 16 };
            4 + count + data.len() * 8 <= data_codewords(*v, level) * 8
        })?;
        let mut bits: Vec<bool> = Vec::with_capacity(data_codewords(version, level) * 8);
        let mut put = |value: u32, n: u32| {
            for i in (0..n).rev() {
                bits.push((value >> i) & 1 == 1);
            }
        };
        put(0b0100, 4);
        put(data.len() as u32, if version <= 9 { 8 } else { 16 });
        for b in data {
            put(u32::from(*b), 8);
        }
        let capacity = data_codewords(version, level) * 8;
        let end = (bits.len() + 4).min(capacity);
        bits.resize(end, false);
        bits.resize(bits.len().div_ceil(8) * 8, false);
        let mut codewords: Vec<u8> = bits
            .chunks(8)
            .map(|c| {
                c.iter()
                    .fold(0u8, |a, b| a << 1 | u8::from(*b))
            })
            .collect();
        for pad in [0xEC, 0x11].iter().cycle() {
            if codewords.len() * 8 >= capacity {
                break;
            }
            codewords.push(*pad);
        }
        let all = interleave(&codewords, version, level);
        let size = version * 4 + 17;
        let mut m = QrMatrix {
            size,
            dark: vec![false; size * size],
            function: vec![false; size * size],
            version,
            mask: 0,
        };
        m.function_patterns(level);
        m.codewords(&all);
        let mut best = (u32::MAX, 0u8);
        for mask in 0..8u8 {
            m.apply_mask(mask);
            m.format(level, mask);
            let penalty = m.penalty();
            if penalty < best.0 {
                best = (penalty, mask);
            }
            m.apply_mask(mask);
        }
        m.mask = best.1;
        m.apply_mask(best.1);
        m.format(level, best.1);
        Some(m)
    }

    /// Modules on each side, without the quiet zone.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Whether the module at column `x` and row `y` is dark.
    pub fn dark(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.size + x]
    }

    /// The data and error correction bits read back from the symbol in placement order, with
    /// the mask removed: what a reader gets before it splits the blocks.
    pub fn read_codewords(&self) -> Vec<u8> {
        let mut copy = QrMatrix {
            size: self.size,
            dark: self.dark.clone(),
            function: self.function.clone(),
            version: self.version,
            mask: self.mask,
        };
        copy.apply_mask(self.mask);
        let mut bits = Vec::new();
        copy.walk(|x, y, m| bits.push(m.dark(x, y)));
        bits.as_chunks::<8>()
            .0
            .iter()
            .map(|c| {
                c.iter()
                    .fold(0u8, |a, b| a << 1 | u8::from(*b))
            })
            .collect()
    }

    fn set(&mut self, x: usize, y: usize, dark: bool) {
        let i = y * self.size + x;
        self.dark[i] = dark;
        self.function[i] = true;
    }

    fn function_patterns(&mut self, level: QrLevel) {
        let size = self.size;
        for i in 0..size {
            self.set(6, i, i % 2 == 0);
            self.set(i, 6, i % 2 == 0);
        }
        for (cx, cy) in [(3, 3), (size - 4, 3), (3, size - 4)] {
            for dy in -4i32..=4 {
                for dx in -4i32..=4 {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    if (0..size as i32).contains(&x) && (0..size as i32).contains(&y) {
                        let d = dx.abs().max(dy.abs());
                        self.set(x as usize, y as usize, d != 2 && d != 4);
                    }
                }
            }
        }
        let align = alignment_positions(self.version);
        let n = align.len();
        for (i, &ax) in align.iter().enumerate() {
            for (j, &ay) in align.iter().enumerate() {
                let corner = (i == 0 && j == 0) || (i == 0 && j == n - 1) || (i == n - 1 && j == 0);
                if corner {
                    continue;
                }
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        self.set(
                            (ax as i32 + dx) as usize,
                            (ay as i32 + dy) as usize,
                            dx.abs().max(dy.abs()) != 1,
                        );
                    }
                }
            }
        }
        self.format(level, 0);
        if self.version >= 7 {
            let bits = qr_version_bits(self.version);
            for i in 0..18 {
                let dark = (bits >> i) & 1 == 1;
                let a = size - 11 + i % 3;
                let b = i / 3;
                self.set(a, b, dark);
                self.set(b, a, dark);
            }
        }
    }

    fn format(&mut self, level: QrLevel, mask: u8) {
        let bits = qr_format_bits(level, mask);
        let bit = |i: u32| (bits >> i) & 1 == 1;
        let size = self.size;
        for i in 0..=5 {
            self.set(8, i, bit(i as u32));
        }
        self.set(8, 7, bit(6));
        self.set(8, 8, bit(7));
        self.set(7, 8, bit(8));
        for i in 9..15 {
            self.set(14 - i, 8, bit(i as u32));
        }
        for i in 0..8 {
            self.set(size - 1 - i, 8, bit(i as u32));
        }
        for i in 8..15 {
            self.set(8, size - 15 + i, bit(i as u32));
        }
        self.set(8, size - 8, true);
    }

    /// Visits the data modules in placement order: two columns at a time from the right, up
    /// and down in turn, skipping the timing column.
    fn walk(&mut self, mut visit: impl FnMut(usize, usize, &mut QrMatrix)) {
        let size = self.size;
        let mut right = size as i32 - 1;
        while right >= 1 {
            if right == 6 {
                right = 5;
            }
            for vert in 0..size {
                for j in 0..2 {
                    let x = (right - j) as usize;
                    let up = (right + 1) & 2 == 0;
                    let y = if up { size - 1 - vert } else { vert };
                    if !self.function[y * size + x] {
                        visit(x, y, self);
                    }
                }
            }
            right -= 2;
        }
    }

    fn codewords(&mut self, data: &[u8]) {
        let mut i = 0;
        let total = data.len() * 8;
        self.walk(|x, y, m| {
            if i < total {
                m.dark[y * m.size + x] = (data[i >> 3] >> (7 - (i & 7))) & 1 == 1;
                i += 1;
            }
        });
    }

    fn apply_mask(&mut self, mask: u8) {
        for y in 0..self.size {
            for x in 0..self.size {
                let invert = match mask {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    _ => ((x + y) % 2 + x * y % 3) % 2 == 0,
                };
                let i = y * self.size + x;
                if invert && !self.function[i] {
                    self.dark[i] = !self.dark[i];
                }
            }
        }
    }

    /// The penalty score of the standard's four rules: runs, blocks, finder like patterns and
    /// the balance of dark and light. The mask with the lowest score is used.
    fn penalty(&self) -> u32 {
        let n = self.size;
        let at = |x: usize, y: usize| self.dark[y * n + x];
        let mut score = 0u32;
        let pattern = [true, false, true, true, true, false, true];
        for line in 0..n {
            for horizontal in [true, false] {
                let get = |k: usize| if horizontal { at(k, line) } else { at(line, k) };
                let mut run = 1;
                for k in 1..n {
                    if get(k) == get(k - 1) {
                        run += 1;
                        if run == 5 {
                            score += 3;
                        } else if run > 5 {
                            score += 1;
                        }
                    } else {
                        run = 1;
                    }
                }
                for k in 0..n.saturating_sub(6) {
                    if (0..7).all(|d| get(k + d) == pattern[d]) {
                        let light = |from: i64| {
                            (from..from + 4).all(|p| p < 0 || p >= n as i64 || !get(p as usize))
                        };
                        if light(k as i64 - 4) || light(k as i64 + 7) {
                            score += 40;
                        }
                    }
                }
            }
        }
        for y in 0..n - 1 {
            for x in 0..n - 1 {
                let c = at(x, y);
                if c == at(x + 1, y) && c == at(x, y + 1) && c == at(x + 1, y + 1) {
                    score += 3;
                }
            }
        }
        let dark = self.dark.iter().filter(|d| **d).count();
        let total = n * n;
        let k = (dark * 20)
            .abs_diff(total * 10)
            .div_ceil(total)
            .saturating_sub(1);
        score + k as u32 * 10
    }
}

/// Centers of the alignment patterns of a version, on both axes.
fn alignment_positions(version: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let n = version / 7 + 2;
    let size = version * 4 + 17;
    let step = if version == 32 {
        26
    } else {
        (version * 4 + n * 2 + 1) / (n * 2 - 2) * 2
    };
    let mut out = vec![6];
    let mut pos = size - 7;
    for _ in 0..n - 1 {
        out.insert(1, pos);
        pos -= step;
    }
    out
}

/// Data codewords split into blocks, each with its error correction, interleaved as the
/// standard places them.
fn interleave(data: &[u8], version: usize, level: QrLevel) -> Vec<u8> {
    let l = level.index();
    let blocks = usize::from(BLOCKS[l][version]);
    let ecc = usize::from(ECC_PER_BLOCK[l][version]);
    let raw = raw_modules(version) / 8;
    let short = blocks - raw % blocks;
    let short_len = raw / blocks;
    let mut split: Vec<Vec<u8>> = Vec::with_capacity(blocks);
    let mut k = 0;
    for i in 0..blocks {
        let len = short_len - ecc + usize::from(i >= short);
        let dat = &data[k..k + len];
        k += len;
        let mut block = dat.to_vec();
        if i < short {
            block.push(0);
        }
        block.extend(qr_error_correction(dat, ecc));
        split.push(block);
    }
    let mut out = Vec::with_capacity(raw);
    for i in 0..split[0].len() {
        for (j, block) in split.iter().enumerate() {
            if i != short_len - ecc || j >= short {
                out.push(block[i]);
            }
        }
    }
    out
}

/// A QR code, drawn as SVG on the server with its four module quiet zone and a text label
/// under it. The text is encoded as UTF-8 bytes at the smallest version that holds it at the
/// chosen level ([`QrLevel::Medium`] unless set), at most [`MAX_QR_BYTES`] bytes; a value that
/// does not fit shows a short notice instead.
///
/// The symbol is black on a white plate in both themes, as readers need. Screen readers hear
/// the figure as an image named by its label, which is also shown under it; without
/// [`QrCode::label`] that is the encoded text.
///
/// ```
/// use zeyn::ui::{QrCode, QrLevel};
/// use zeyn::{Component, I18n, Render};
///
/// let qr = QrCode::new("https://example.com/orders/42").level(QrLevel::Quartile).label("Order 42");
/// let mut html = String::new();
/// qr.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains(r#"role="img""#) && html.contains(">Order 42</figcaption>"));
/// ```
pub struct QrCode {
    value: String,
    level: QrLevel,
    label: Option<Text>,
    size: u32,
}

impl QrCode {
    pub fn new(value: impl Into<String>) -> Self {
        QrCode {
            value: value.into(),
            level: QrLevel::Medium,
            label: None,
            size: 160,
        }
    }

    /// How much damage the code survives; higher levels make a larger symbol.
    pub fn level(mut self, level: QrLevel) -> Self {
        self.level = level;
        self
    }

    /// The text under the code and the name screen readers hear, instead of the value.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Width and height in CSS pixels, 64 to 1024; 160 unless set.
    pub fn size(mut self, px: u32) -> Self {
        self.size = px.clamp(64, 1024);
        self
    }

    /// The SVG of the symbol, or `None` when the value does not fit.
    pub fn svg(&self) -> Option<String> {
        if self.value.len() > MAX_QR_BYTES {
            return None;
        }
        let m = QrMatrix::encode(self.value.as_bytes(), self.level)?;
        let n = m.size();
        let full = n as u32 + 2 * QR_QUIET;
        let mut path = String::with_capacity(n * n);
        for y in 0..n {
            let mut x = 0;
            while x < n {
                if !m.dark(x, y) {
                    x += 1;
                    continue;
                }
                let run = (x..n)
                    .take_while(|&k| m.dark(k, y))
                    .count();
                let _ = write!(
                    path,
                    "M{} {}h{run}v1h-{run}z",
                    x as u32 + QR_QUIET,
                    y as u32 + QR_QUIET
                );
                x += run;
            }
        }
        Some(format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {full} {full}" width="{0}" height="{0}" shape-rendering="crispEdges" focusable="false"><rect width="{full}" height="{full}" fill="#fff"/><path fill="#000" d="{path}"/></svg>"##,
            self.size
        ))
    }
}

impl Component for QrCode {
    fn render(&self, out: &mut String, r: &Render) {
        match self.svg() {
            Some(svg) => figure(
                out,
                r,
                "z-barcode-qr",
                &svg,
                &label_of(&self.label, &self.value, r),
                "qr",
            ),
            None => refused(out, r, "qr_too_long"),
        }
    }

    fn schema(&self) -> Json {
        json!({ "type": "qr_code", "value": self.value, "label": self.label, "level": format!("{:?}", self.level) })
    }
}

#[cfg(test)]
mod tests {
    use super::CODE128;

    #[test]
    fn every_code_128_symbol_is_eleven_modules_with_even_bars_and_unique() {
        for (i, s) in CODE128.iter().enumerate() {
            let w: Vec<u32> = s
                .bytes()
                .map(|b| u32::from(b - b'0'))
                .collect();
            let total: u32 = w.iter().sum();
            assert_eq!(total, if i == 106 { 13 } else { 11 }, "symbol {i}");
            let bars: u32 = w.iter().step_by(2).sum();
            assert_eq!(bars % 2, 0, "symbol {i} has an even bar width sum");
            assert!(w.iter().all(|x| (1..=4).contains(x)), "symbol {i}");
            assert_eq!(
                CODE128
                    .iter()
                    .filter(|o| *o == s)
                    .count(),
                1,
                "symbol {i}"
            );
        }
    }
}
