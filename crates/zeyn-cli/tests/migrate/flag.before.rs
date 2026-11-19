pub fn app() -> App {
    App::new("shop")
        .theme_picker(false)
        .secure_cookies(true)
        // stays
        .scrollbar(false)
        .realtime(live)
}

pub fn grid() -> DataGrid<Order> {
    DataGrid::of(find)
        .sort_by(Order::date, true)
        .sort_by(Order::code,false)
        .sort_by(Order::total, newest_first)
}
