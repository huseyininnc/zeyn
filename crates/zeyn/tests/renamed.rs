//! The pre 0.0.2 names still compile for one release, with a deprecation warning.

#![allow(deprecated)]

use zeyn::prelude::*;

#[entity(table = "things")]
pub struct Thing {
    #[key]
    pub id: i64,
    pub name: String,
}

#[action(permission = "things.read")]
pub async fn find_things(q: GridQuery) -> Result<Page<Thing>> {
    Ok(q.apply(vec![Thing {
        id: 1,
        name: "One".into(),
    }]))
}

#[test]
fn old_grid_names_still_build_a_data_grid() {
    let grid: Grid<Thing> = Grid::of(find_things)
        .column(Thing::name)
        .mode(GridMode::Server);
    let _: DataGrid<Thing> = grid;
    let _: zeyn::GridColumn<Thing, String> = DataGrid::of(find_things).column(Thing::name);
}
