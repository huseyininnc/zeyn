use zeyn::{DataGrid, DataQuery};

/// `DataGrid::of(find)` shows rows.
fn list(q: DataQuery) -> DataGrid<P> { DataGrid::of(find).mode(DataMode::Server).title("Grid") }

struct MyGrid; impl GridSource for MyGrid {}

const RAW: &str = r#"Grid "quoted" Grid"#; const C: char = 'G';

fn menu(m: Menu) -> Menu {
    m.group("Reports", Icon::ChartBar).icon(Icon::Trash) // Icon::Trash stays readable
}

fn other() { Method::Delete; Icon::ChartBar; let Chart = 1; }
