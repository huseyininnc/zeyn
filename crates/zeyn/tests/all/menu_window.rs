//! Menu items that open a screen as a window (`Menu::window`): their kind in the menu and the app
//! description, the check of their screen, and the side menu button that runs `open_window`
//! for users who may open the screen.

use zeyn::MenuTarget;
use zeyn::prelude::*;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AboutParams {
    pub tab: String,
}

impl zeyn::Params for AboutParams {}

#[screen(public)]
pub fn about(p: AboutParams) -> Screen {
    Screen::window(format!("About {}", p.tab))
}

#[screen(permission = "logs.read")]
pub fn log_viewer() -> Screen {
    Screen::window("Logs")
}

#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

/// A feature whose `Help` group holds a page item, a window item with params and, in a nested
/// group, a window item that needs a permission.
fn feature() -> Feature {
    Feature::new("help")
        .screen(home)
        .screen(about)
        .screen(log_viewer)
        .menu(|m| {
            m.group("Help", Icon::Info)
                .open()
                .item("Home", home)
                .window(
                    "About",
                    about,
                    AboutParams {
                        tab: "<team>".into(),
                    },
                )
                .icon(Icon::Info)
                .badge("New")
                .sub("Tools", |m| m.window("Logs", log_viewer, ()))
        })
}

#[test]
fn window_items_have_the_window_kind_and_their_params() {
    let f = feature();
    let items = &f.menu_groups()[0].items;
    assert_eq!(items[0].target, MenuTarget::Page);
    assert!(!items[0].is_window());
    assert!(items[1].is_window());
    assert_eq!(
        items[1].target,
        MenuTarget::Window {
            params: serde_json::json!({ "tab": "<team>" })
        }
    );
    let json = serde_json::to_value(&items[1]).unwrap();
    assert_eq!(json["kind"], "window");
    assert_eq!(json["screen"], "about");
    assert_eq!(json["params"]["tab"], "<team>");
    assert!(items[1].icon.is_some() && items[1].badge.is_some());
    assert_eq!(serde_json::to_value(&items[0]).unwrap()["kind"], "page");
    let nested = &f.menu_groups()[0].groups[0].items[0];
    assert!(nested.is_window());
    assert_eq!(nested.screen, "log_viewer");
}

#[test]
fn find_returns_page_items_only() {
    let f = feature();
    assert!(
        f.menu_groups()[0]
            .find("home")
            .is_some()
    );
    assert!(
        f.menu_groups()[0]
            .find("about")
            .is_none()
    );
    assert_eq!(f.menu_groups()[0].all_items().len(), 3);
}

#[test]
fn window_items_pass_the_check_without_a_route() {
    assert!(feature().check().is_ok());
    assert!(
        App::new("T")
            .feature(feature())
            .check()
            .is_ok()
    );
}

#[test]
fn a_window_item_may_open_a_screen_of_another_feature() {
    let app = App::new("T")
        .feature(Feature::new("a").screen(about))
        .feature(Feature::new("b").menu(|m| {
            m.group("Help", Icon::Info)
                .window("About", about, AboutParams::default())
        }));
    assert!(app.check().is_ok(), "{:?}", app.check());
}

#[test]
fn a_window_item_whose_screen_no_feature_registers_is_a_problem() {
    let problems = App::new("T")
        .feature(Feature::new("a").menu(|m| {
            m.group("Help", Icon::Info)
                .sub("Deep", |m| m.window("Logs", log_viewer, ()))
        }))
        .check()
        .unwrap_err();
    let p = problems
        .iter()
        .find(|p| p.contains("Z0418"))
        .expect("Z0418");
    assert!(p.contains("menu item `Logs` opens screen `log_viewer` as a window"));
}

#[test]
fn describe_lists_the_kind_and_params_of_menu_items() {
    let d = App::new("T")
        .feature(feature())
        .describe();
    let items = &d["features"][0]["menu"][0]["items"];
    assert_eq!(items[0]["kind"], "page");
    assert_eq!(items[1]["kind"], "window");
    assert_eq!(items[1]["params"]["tab"], "<team>");
    assert_eq!(
        d["features"][0]["menu"][0]["groups"][0]["items"][0]["kind"],
        "window"
    );
}
