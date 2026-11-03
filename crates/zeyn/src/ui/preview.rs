//! Hover previews of cards and list rows ([`super::ImageCard::preview`],
//! [`super::ListItem::preview`]): the attribute that names the screen `preview.js` loads.

use super::widgets::encode;
use crate::render::escape;
use serde_json::Value as Json;

/// A screen shown in a preview: its name and params.
pub(crate) type PreviewTarget = Option<(&'static str, Json)>;

/// The ` data-zeyn-preview="..."` attribute of a card link: the address of the screen fragment
/// below the app base, or nothing without a preview.
pub(crate) fn preview_attr(target: &PreviewTarget) -> String {
    let Some((screen, params)) = target else { return String::new() };
    let query = if params.is_null() { String::new() } else { format!("?params={}", encode(&params.to_string())) };
    format!(r#" data-zeyn-preview="{}""#, escape(&format!("/_zeyn/screen/{screen}{query}")))
}
