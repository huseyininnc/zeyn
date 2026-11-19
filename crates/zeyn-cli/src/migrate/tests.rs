use super::*;

/// A step that is not in the registry, for the general call shape.
const SHAPES: Step = Step {
    version: "9.0.0",
    changes: &[
        Change::Call {
            method: "foo",
            args: 2,
            to: ".bar({1}).baz({0})",
        },
        Change::Call {
            method: "gone",
            args: 0,
            to: "",
        },
    ],
};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/migrate")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n")
}

/// Runs the steps the way `zeyn migrate` does and returns the text, the notes and the TODOs.
fn migrate(text: &str, steps: &[&Step]) -> (String, Vec<String>, Vec<String>) {
    let mut report = Report::default();
    let out = migrate_file("x.rs", text, steps, &mut report);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let notes = report
        .changes
        .iter()
        .map(|c| format!("{}: {}", c["line"], c["change"].as_str().unwrap()))
        .collect();
    let todos = report
        .todos
        .iter()
        .map(|t| format!("{}: {}", t["line"], t["reason"].as_str().unwrap()))
        .collect();
    (out, notes, todos)
}

/// Checks a fixture pair and that a second run changes nothing.
fn check(kind: &str, steps: &[&Step]) -> (Vec<String>, Vec<String>) {
    let before = fixture(&format!("{kind}.before.rs"));
    let after = fixture(&format!("{kind}.after.rs"));
    let (out, notes, todos) = migrate(&before, steps);
    assert_eq!(out, after, "{kind}: {notes:#?}");
    let (again, more, _) = migrate(&out, steps);
    assert_eq!(again, out, "{kind}: a second run changed {more:?}");
    assert!(more.is_empty());
    (notes, todos)
}

fn step(version: &str) -> &'static Step {
    STEPS
        .iter()
        .find(|s| s.version == version)
        .unwrap()
}

#[test]
fn renames_identifiers_and_comments_but_not_strings_or_longer_names() {
    let (notes, todos) = check("rename", &[step("0.0.6"), step("0.0.9")]);
    assert_eq!(notes.len(), 10, "{notes:#?}");
    assert!(notes.contains(&"1: `Grid` -> `DataGrid`".to_string()));
    assert!(notes.contains(&"11: `Icon::Delete` -> `Icon::Trash`".to_string()));
    assert!(todos.is_empty());
}

#[test]
fn reshapes_calls_keeps_comments_and_flags_calls_with_comments_between_arguments() {
    let (notes, todos) = check("call", &[&SHAPES]);
    assert_eq!(
        notes,
        [
            "4: `.foo(first(1, 2), \"second\")` -> `.bar(\"second\").baz(first(1, 2))`",
            "10: `.gone()` -> ``",
            "14: `.foo(y.foo(1, 2), 3)` -> `.bar(3).baz(y.foo(1, 2))`",
            "14: `.foo(1, 2)` -> `.bar(2).baz(1)`",
        ]
    );
    assert_eq!(
        todos,
        [
            "6: comments or generic types between the arguments: rewrite `.foo(..)` as `.bar({1}).baz({0})` by hand"
        ]
    );
}

#[test]
fn flags_become_named_methods_and_other_values_need_a_person() {
    let (notes, todos) = check("flag", &[step("0.0.11")]);
    assert_eq!(notes.len(), 6, "{notes:#?}");
    assert!(notes.contains(&"3: `.theme_picker(false)` -> `.no_theme_picker()`".to_string()));
    assert!(notes.contains(&"6: `.scrollbar(false)` -> ``".to_string()));
    assert_eq!(
        todos,
        [
            "14: `.sort_by` no longer takes a bool: write `.sort_by_desc(Order::total)` for true and `.sort_by(Order::total)` for false"
        ]
    );
}

#[test]
fn moves_simple_use_paths_and_code_paths_and_leaves_groups_to_a_person() {
    let (notes, todos) = check("use", &[step("0.0.11")]);
    assert_eq!(notes.len(), 4, "{notes:#?}");
    assert_eq!(
        todos,
        [
            "3: `zeyn::rank::ReorderRequest` moved to `zeyn::ReorderRequest`: change this `use` by hand"
        ]
    );
}

#[test]
fn review_patterns_are_reported_with_their_reason_and_change_nothing() {
    let (notes, todos) = check("review", &[step("0.0.11")]);
    assert!(notes.is_empty());
    let lines: Vec<&str> = todos
        .iter()
        .map(|t| t.split(':').next().unwrap())
        .collect();
    assert_eq!(
        lines,
        ["1", "3", "6", "10", "14", "19", "21", "23"],
        "{todos:#?}"
    );
    assert!(todos[2].contains("ReorderRequest::new(key, after, before)"));
    assert!(todos[3].contains("POST /_zeyn/account/logout"));
    assert!(todos[4].contains("ImportPreview"));
    assert!(todos[5].contains("RateStore::refund"));
    assert!(todos[6].contains("DocStore::allow"));
    assert!(todos[7].contains("PageCheck::problem"));
}

#[test]
fn a_file_that_does_not_parse_is_an_error_with_its_line() {
    let mut report = Report::default();
    let text = "fn main() {\n    let = ;\n}\n";
    let out = migrate_file("bad.rs", text, &[step("0.0.11")], &mut report);
    assert_eq!(out, text);
    let error = report.errors[0]["error"]
        .as_str()
        .unwrap();
    assert!(error.starts_with("2:"), "{error}");
}

#[test]
fn a_project_with_a_file_that_does_not_parse_is_left_as_it_was() {
    let root = std::env::temp_dir().join(format!("zeyn-migrate-bad-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let good = "fn f(g: Grid) {}
";
    std::fs::write(root.join("src/good.rs"), good).unwrap();
    std::fs::write(
        root.join("src/bad.rs"),
        "fn main() {
    let = ;
}
",
    )
    .unwrap();
    let report = run(&root, Some("0.0.5"), None, false).unwrap();
    assert_eq!(report.errors.len(), 1);
    assert!(
        report
            .to_text(false)
            .contains("nothing written")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/good.rs")).unwrap(),
        good
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn versions_compare_by_number_and_steps_run_in_order() {
    assert!(version("0.0.10").unwrap() > version("0.0.9").unwrap());
    assert_eq!(version("0.1.0-alpha.1").unwrap(), (0, 1, 0));
    assert!(version("next").is_err());
    let versions: Vec<_> = STEPS
        .iter()
        .map(|s| version(s.version).unwrap())
        .collect();
    assert!(versions.is_sorted(), "STEPS must be oldest first");
    assert_eq!(latest(), "0.0.11");
}

#[test]
fn manifests_move_zeyn_crates_only_and_keep_their_operator() {
    let text = "[dependencies]\nzeyn = { version = \"^0.0.9\", features = [\"postgres\"] }\nzeyn-macros = \"0.0.9\"\nzeyn-extra = \"0.0.9\"\nserde = \"1\"\nzeyn-island = { path = \"../island\" }\n";
    let out = bump_manifest(text, (0, 0, 11), "0.0.11");
    assert_eq!(
        out,
        "[dependencies]\nzeyn = { version = \"^0.0.11\", features = [\"postgres\"] }\nzeyn-macros = \"0.0.11\"\nzeyn-extra = \"0.0.9\"\nserde = \"1\"\nzeyn-island = { path = \"../island\" }\n"
    );
    assert_eq!(bump_manifest(&out, (0, 0, 11), "0.0.11"), out);
}

#[test]
fn a_project_migrates_with_a_stable_report_and_a_dry_run_writes_nothing() {
    let root = std::env::temp_dir().join(format!("zeyn-migrate-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let manifest = "[package]\nname = \"shop\"\n\n[dependencies]\nzeyn = \"0.0.9\"\n";
    let main = "fn main() {\n    let _ = App::new(\"shop\").theme_picker(false);\n    let _ = \"/_zeyn/auth/logout\";\n}\n";
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        "[[package]]\nname = \"zeyn\"\nversion = \"0.0.9\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"), main).unwrap();

    let dry = run(&root, None, None, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("src/main.rs")).unwrap(),
        main
    );
    assert!(
        dry.diff
            .contains("-    let _ = App::new(\"shop\").theme_picker(false);\n+    let _ = App::new(\"shop\").no_theme_picker();\n"),
        "{}",
        dry.diff
    );
    assert!(
        dry.diff
            .contains("-zeyn = \"0.0.9\"\n+zeyn = \"0.0.11\"\n")
    );

    let report = run(&root, None, None, false).unwrap();
    assert_eq!(
        report.to_json(false),
        json!({
            "from": "0.0.9",
            "to": "0.0.11",
            "dry_run": false,
            "steps": ["0.0.11"],
            "files": ["Cargo.toml", "src/main.rs"],
            "changes": [
                { "file": "Cargo.toml", "line": 0, "step": "0.0.11", "change": "Zeyn dependencies -> 0.0.11" },
                { "file": "src/main.rs", "line": 2, "step": "0.0.11", "change": "`.theme_picker(false)` -> `.no_theme_picker()`" }
            ],
            "todos": [
                { "file": "src/main.rs", "line": 3, "step": "0.0.11", "reason": "`GET /_zeyn/auth/logout` no longer ends a session: give sign out links `data-zeyn-signout` so they post with the CSRF token" }
            ],
            "errors": []
        })
    );
    assert!(
        std::fs::read_to_string(root.join("Cargo.toml"))
            .unwrap()
            .contains("zeyn = \"0.0.11\"")
    );
    let again = run(&root, Some("0.0.9"), None, false).unwrap();
    assert!(again.files.is_empty(), "{:?}", again.changes);
    assert!(run(&root, Some("0.0.11"), Some("0.0.9"), true).is_err());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn every_example_parses_and_is_already_current() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let report = run(&examples, Some("0.0.5"), None, true).unwrap();
    assert!(report.errors.is_empty(), "{:#?}", report.errors);
    assert!(report.changes.is_empty(), "{:#?}", report.changes);
}

#[test]
fn a_new_project_parses_and_is_already_current() {
    let base = std::env::temp_dir().join(format!("zeyn-migrate-new-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).unwrap();
    for (name, lib) in [("app", false), ("shared", true)] {
        let root = base.join(name);
        crate::new_project_at(&root, name, None, lib, false).unwrap();
        let report = run(&root, Some("0.0.5"), None, true).unwrap();
        assert!(report.errors.is_empty(), "{:#?}", report.errors);
        assert!(
            report
                .changes
                .iter()
                .all(|c| c["file"] == "Cargo.toml"),
            "{:#?}",
            report.changes
        );
        assert!(report.todos.is_empty(), "{:#?}", report.todos);
    }
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn job_structs_get_default_rest_and_job_stores_are_flagged() {
    let (notes, todos) = check("jobs", &[step("0.0.11")]);
    assert_eq!(notes.len(), 6, "{notes:#?}");
    assert!(notes[0].contains("`NewJob` literal gets `..Default::default()`"));
    let lines: Vec<&str> = todos
        .iter()
        .map(|t| t.split(':').next().unwrap())
        .collect();
    assert_eq!(lines, ["1", "49", "53"], "{todos:#?}");
    assert!(todos[1].contains("add_trigger"));
    assert!(todos[2].contains("migrate_job_tables"));
}
