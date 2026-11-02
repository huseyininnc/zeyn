use zeyn::prelude::*;

/// A dish or drink on the menu.
#[entity(table = "menu_items")]
pub struct MenuItem {
    #[key]
    pub id: MenuItemId,
    #[rule(required, max_len = 60)]
    pub name: String,
    pub category: Category,
    #[rule(min = 0)]
    pub price: Money,
    #[rule(min = 0, max = 9999)]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}

/// Where an item sits on the menu.
#[choice]
pub enum Category {
    Soups,
    Mains,
    Desserts,
    Drinks,
}
