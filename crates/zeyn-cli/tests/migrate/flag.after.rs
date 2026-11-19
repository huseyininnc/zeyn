pub fn app() -> App {
    App::new("shop")
        .no_theme_picker()
        .secure_cookies()
        // stays
        .socket(live)
}

pub fn grid() -> DataGrid<Order> {
    DataGrid::of(find)
        .sort_by_desc(Order::date)
        .sort_by(Order::code)
        .sort_by(Order::total, newest_first)
}
