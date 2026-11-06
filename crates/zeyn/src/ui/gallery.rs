//! [`ProductGallery`]: a main image with thumbnails, zoom, swipe and a full screen view.

use super::catalog::safe_image;
use super::layout::into_node;
use super::{Given, Image, dom_id};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(ProductGallery);

/// Most images one gallery shows; more are left out.
pub const MAX_GALLERY_IMAGES: usize = 24;

/// The photos of a product: one large image at a time with thumbnails under it. Phones swipe
/// between images (native scroll snapping), a mouse zooms the image under the pointer, and a
/// press opens a full screen view, a modal dialog that keeps focus inside, closes with Escape
/// and gives focus back. The slides follow the W3C APG carousel pattern: a named region, each
/// slide "2 of 5", thumbnails as buttons with `aria-current`, the arrow keys on the image move
/// between slides.
///
/// Images are [`Image`]s with their alt text and size, so they keep their placeholder, low
/// resolution file and fallback. Only the first is in the page as an image, eager and with
/// `fetchpriority="high"`, because it is the largest thing a product page shows; the others wait
/// in inert templates and `gallery.js` (loaded only on pages with a gallery) puts in the image on
/// screen and its two neighbors, so a gallery of 12 photos loads three. Thumbnails are small
/// files of their own ([`ProductGallery::thumb`]); without them the gallery shows numbered dots.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::ProductGallery;
///
/// let gallery = ProductGallery::new("Linen shirt")
///     .image(Image::new("/p/42-1.webp").alt("Front").size(800, 800))
///     .thumb("/p/42-1-thumb.webp")
///     .image(Image::new("/p/42-2.webp").alt("Back").size(800, 800))
///     .thumb("/p/42-2-thumb.webp");
/// ```
pub struct ProductGallery {
    label: Text,
    images: Vec<(Image<Given, Given>, Option<String>)>,
}

impl ProductGallery {
    /// A gallery named `label` for screen readers, such as the product name.
    pub fn new(label: impl Into<Text>) -> Self {
        ProductGallery {
            label: label.into(),
            images: Vec::new(),
        }
    }

    /// An image, at most [`MAX_GALLERY_IMAGES`].
    pub fn image(mut self, image: Image<Given, Given>) -> Self {
        if self.images.len() < MAX_GALLERY_IMAGES {
            let image = if self.images.is_empty() {
                image.priority()
            } else {
                image
            };
            self.images.push((image, None));
        }
        self
    }

    /// The thumbnail of the image added last: a small file (a path of the app, an `https:`
    /// address or a `data:image/` URI), loaded lazily.
    pub fn thumb(mut self, src: impl Into<String>) -> Self {
        if let Some(last) = self.images.last_mut() {
            let src = src.into();
            last.1 = safe_image(&src).then_some(src);
        }
        self
    }
}

impl Component for ProductGallery {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "gallery");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.shop.gallery.{key}")));
        let count = self.images.len().to_string();
        out.push_str(&format!(
            r#"<section class="z-gallery" id="{id}" aria-roledescription="{}" aria-label="{}" data-zeyn-gallery data-text-close="{}" data-text-prev="{}" data-text-next="{}" data-text-zoom="{}"><div class="z-gallery-main"><div class="z-gallery-track" tabindex="0" aria-label="{}">"#,
            r.text(&Text::key("zeyn.ui.hero.carousel")),
            r.text(&self.label),
            t("close"),
            t("prev"),
            t("next"),
            t("zoom"),
            t("swipe"),
        ));
        for (i, (image, _)) in self.images.iter().enumerate() {
            let of = r.text(
                &Text::key("zeyn.ui.hero.slide_of")
                    .arg("n", (i + 1).to_string())
                    .arg("count", count.clone()),
            );
            out.push_str(&format!(
                r#"<div class="z-gallery-slide" role="group" aria-roledescription="{}" aria-label="{of}" data-index="{i}">"#,
                r.text(&Text::key("zeyn.ui.hero.slide")),
            ));
            if i == 0 {
                image.render(out, r);
            } else {
                out.push_str("<template>");
                image.render(out, r);
                out.push_str("</template>");
            }
            out.push_str("</div>");
        }
        out.push_str(&format!(
            r#"</div><button type="button" class="z-gallery-open" aria-haspopup="dialog" aria-controls="{id}">{}</button></div>"#,
            t("full"),
        ));
        if self.images.len() > 1 {
            out.push_str(&format!(
                r#"<div class="z-gallery-thumbs" role="group" aria-label="{}">"#,
                t("choose")
            ));
            for (i, (_, thumb)) in self.images.iter().enumerate() {
                let name = r.text(
                    &Text::key("zeyn.ui.shop.gallery.show")
                        .arg("n", (i + 1).to_string())
                        .arg("count", count.clone()),
                );
                let current = if i == 0 {
                    r#" aria-current="true""#
                } else {
                    ""
                };
                let inside = match thumb {
                    Some(src) => format!(
                        r#"<img src="{}" alt="" width="64" height="64" loading="lazy" decoding="async">"#,
                        escape(src)
                    ),
                    None => String::from(r#"<span class="z-gallery-dot"></span>"#),
                };
                let class = if thumb.is_some() {
                    "z-gallery-thumb"
                } else {
                    "z-gallery-thumb z-gallery-thumb-dot"
                };
                out.push_str(&format!(
                    r#"<button type="button" class="{class}" data-index="{i}" aria-label="{name}"{current}>{inside}</button>"#
                ));
            }
            out.push_str("</div>");
        }
        out.push_str("</section>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "product_gallery",
            "label": self.label,
            "images": self
                .images
                .iter()
                .map(|(i, thumb)| json!({ "image": i.schema(), "thumb": thumb }))
                .collect::<Vec<_>>(),
        })
    }
}
