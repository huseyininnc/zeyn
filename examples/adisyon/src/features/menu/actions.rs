use super::models::MenuItem;
use crate::restaurant::Restaurant;
use zeyn::prelude::*;

/// The menu, for the order screen, the menu list and the API.
#[action(permission = "pos.order")]
pub async fn find_menu(q: DataQuery, restaurant: Restaurant) -> Result<Page<MenuItem>> {
    Ok(q.apply(restaurant.menu()?))
}

/// Changes a menu item's price, stock or availability.
#[action(permission = "pos.menu", audit)]
pub async fn save_menu_item(item: Valid<MenuItem>, restaurant: Restaurant) -> Result<MenuItem> {
    restaurant.save_item(item.into_inner())
}
