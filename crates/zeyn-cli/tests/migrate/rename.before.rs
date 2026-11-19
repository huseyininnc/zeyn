use zeyn::{Grid, GridQuery};

/// `Grid::of(find)` shows rows.
fn list(q: GridQuery) -> Grid<P> { Grid::of(find).mode(GridMode::Server).title("Grid") }

struct MyGrid; impl GridSource for MyGrid {}

const RAW: &str = r#"Grid "quoted" Grid"#; const C: char = 'G';

fn menu(m: Menu) -> Menu {
    m.group("Reports", Icon::Chart).icon(Icon::Delete) // Icon::Delete stays readable
}

fn other() { Method::Delete; Icon::ChartBar; let Chart = 1; }
