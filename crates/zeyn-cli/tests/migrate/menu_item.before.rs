use zeyn::{MenuItem, Text};

fn item() -> MenuItem {
    MenuItem {
        label: Text::from("Orders"),
        screen: "orders",
        icon: None,
        badge: None,
        count: None,
        topics: Vec::new(),
    }
}
