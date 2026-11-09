//! Lookup examples: pick a record from a server side search, and a lookup whose options depend
//! on another field.

use crate::data::{ProductId, find_products};
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{LookupItem, LookupQuery};

/// An order line that refers to a product.
#[model]
pub struct Pick {
    #[rule(required)]
    pub product: Option<ProductId>,
}

/// Accepts the pick. Nothing is stored.
#[action(public)]
pub async fn save_pick(pick: Valid<Pick>) -> Result<bool> {
    Ok(pick.product.is_some())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Pick::default())
        .lookup(Pick::product, find_products)
        .field(Pick::product)
        .submit("Save", save_pick)
        .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
        .into()
}

/// An address: a country, then a city of that country.
#[model]
pub struct Place {
    #[rule(required)]
    pub country: Option<i64>,
    #[rule(required)]
    pub city: Option<i64>,
}

/// Sample cities: id, country id and name.
const CITIES: [(i64, i64, &str); 8] = [
    (11, 1, "İstanbul"),
    (12, 1, "İzmir"),
    (13, 1, "Ankara"),
    (21, 2, "Berlin"),
    (22, 2, "Hamburg"),
    (31, 3, "Tokyo"),
    (32, 3, "Osaka"),
    (33, 3, "Kyoto"),
];

/// Sample countries.
#[action(public)]
pub async fn find_countries(q: LookupQuery) -> Result<Page<LookupItem>> {
    Ok(q.apply(vec![
        LookupItem::new(1, "Türkiye"),
        LookupItem::new(2, "Germany"),
        LookupItem::new(3, "Japan"),
    ]))
}

/// The cities of the country in `q.parent`; none before a country is chosen.
#[action(public)]
pub async fn find_cities(q: LookupQuery) -> Result<Page<LookupItem>> {
    let country = q.parent_id();
    let cities = CITIES
        .iter()
        .filter(|(_, c, _)| Some(*c) == country)
        .map(|(id, _, name)| LookupItem::new(*id, *name))
        .collect();
    Ok(q.apply(cities))
}

/// Accepts the place. Nothing is stored.
#[action(public)]
pub async fn save_place(place: Valid<Place>) -> Result<bool> {
    Ok(place.city.is_some())
}

pub fn country_city(_ctx: &Ctx) -> Node {
    Form::of(Place::default())
        .lookup(Place::country, find_countries)
        .lookup(Place::city, find_cities)
        .depends_on(Place::city, Place::country)
        .submit("Save", save_place)
        .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
        .into()
}

/// What the catalog shows of `LookupItem`.
pub const ENTRY: Entry = Entry {
    name: "Lookup",
    docs: "components/controls",
    api: &["LookupItem", "LookupQuery"],
    overview: &[ex!(overview)],
    examples: &[ex!(country_city)],
};
