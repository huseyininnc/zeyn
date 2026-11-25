//! The integration tests of the framework, built as one test binary so the test files are compiled
//! and linked once. Run one area with `cargo test -p zeyn --test all <module>::`.
//! The tests that install a counting global allocator stay in their own files under `tests/`.

#[cfg(feature = "accounts")]
mod accounts;
#[cfg(feature = "accounts")]
mod accounts_passkeys;
#[cfg(all(feature = "accounts", feature = "postgres"))]
mod accounts_sql;
mod ai;
#[cfg(feature = "assistant")]
mod assistant;
mod check_problems;
mod code_measure;
mod devtools;
mod empty_states;
mod enterprise;
mod files;
#[cfg(feature = "image")]
mod files_image;
mod grid;
mod icons;
mod inspect;
mod interceptors;
mod inventory;
#[cfg(feature = "islands")]
mod islands;
mod jobs_store;
mod license;
#[cfg(feature = "email")]
mod mail;
mod menu_source;
mod menu_window;
#[cfg(feature = "metronome")]
mod metronome;
mod navigation;
#[cfg(feature = "oidc")]
mod oidc;
mod oracle;
#[cfg(feature = "pdf")]
mod pdf;
mod platform;
mod postgres;
mod public_pages;
mod rank;
#[cfg(feature = "realtime")]
mod realtime;
mod redis;
mod render_checks;
mod request_cost;
#[cfg(feature = "resumable")]
mod resumable;
mod rtl;
mod search;
mod server;
mod site;
mod size_budgets;
#[cfg(feature = "socket-client")]
mod socket_client;
#[cfg(any(feature = "postgres", feature = "oracle"))]
mod sql_common;
mod testing;
mod theme;
mod time;
mod topics;
#[cfg(feature = "totp")]
mod totp;
mod ui_admin;
mod ui_builders;
mod ui_catalog;
mod ui_chat;
mod ui_code;
mod ui_commerce;
mod ui_content;
mod ui_controls;
mod ui_data;
mod ui_date;
mod ui_document;
mod ui_embed;
mod ui_engage;
mod ui_file_drop;
mod ui_inputs;
mod ui_layout;
mod ui_lines;
#[cfg(feature = "markdown")]
mod ui_markdown;
mod ui_media;
mod ui_menus;
mod ui_modules;
mod ui_multi;
mod ui_overlay;
mod ui_pro;
mod ui_schema;
mod ui_social;
mod ui_surface;
mod ui_widgets;
#[cfg(feature = "xlsx")]
mod xlsx;
