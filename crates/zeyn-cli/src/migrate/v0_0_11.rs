//! The version after 0.0.10: the page socket, builder methods without `bool` flags, nested
//! menu groups and sign out by `POST` only. Rename this module when the version is released
//! under another number.

use super::{Change, Pattern, Step};

pub const STEP: Step = Step {
    version: "0.0.11",
    changes: &[
        Change::Rename("Realtime", "ZeynSocket"),
        Change::Rename("HubStats", "SocketStats"),
        Change::Rename("Hub", "Socket"),
        Change::Call {
            method: "realtime",
            args: 1,
            to: ".socket({0})",
        },
        Change::Call {
            method: "hub",
            args: 0,
            to: ".socket()",
        },
        Change::Flag {
            method: "sort_by",
            args: 2,
            on: ".sort_by_desc({0})",
            off: ".sort_by({0})",
        },
        Change::Flag {
            method: "sort_option",
            args: 3,
            on: ".sort_option_desc({0}, {1})",
            off: ".sort_option({0}, {1})",
        },
        Change::Flag {
            method: "scrollbar",
            args: 1,
            on: ".scrollbar()",
            off: "",
        },
        Change::Flag {
            method: "secure_cookies",
            args: 1,
            on: ".secure_cookies()",
            off: "",
        },
        Change::Flag {
            method: "refresh_tokens",
            args: 1,
            on: ".refresh_tokens()",
            off: "",
        },
        Change::Flag {
            method: "admin",
            args: 1,
            on: ".admin()",
            off: "",
        },
        Change::Flag {
            method: "theme_picker",
            args: 1,
            on: "",
            off: ".no_theme_picker()",
        },
        Change::Flag {
            method: "browser_context_menu",
            args: 1,
            on: "",
            off: ".no_browser_context_menu()",
        },
        Change::Flag {
            method: "server",
            args: 1,
            on: "",
            off: ".no_server()",
        },
        Change::Flag {
            method: "webmcp",
            args: 1,
            on: "",
            off: ".no_webmcp()",
        },
        Change::Flag {
            method: "launcher",
            args: 1,
            on: "",
            off: ".no_launcher()",
        },
        Change::Flag {
            method: "registration",
            args: 1,
            on: "",
            off: ".no_registration()",
        },
        Change::Use("zeyn::rank::ReorderRequest", "zeyn::ReorderRequest"),
        Change::Review(
            Pattern::Struct("ReorderRequest"),
            "`ReorderRequest` has new fields (`rest`, `list`, `from`): build it with `ReorderRequest::new(key, after, before)`",
        ),
        Change::Review(
            Pattern::Name("MenuGroup"),
            "`MenuGroup::icon` is now `Option<Icon>` and groups nest (`groups`, `open`): wrap icons in `Some(..)` and handle `None` where the field is read",
        ),
        Change::Review(
            Pattern::Text("/_zeyn/account/signout"),
            "`GET /_zeyn/account/signout` is gone: sign out with `POST /_zeyn/account/logout` and the CSRF token, or give the link `data-zeyn-signout`",
        ),
        Change::Review(
            Pattern::Text("/_zeyn/auth/logout"),
            "`GET /_zeyn/auth/logout` no longer ends a session: give sign out links `data-zeyn-signout` so they post with the CSRF token",
        ),
        Change::Review(
            Pattern::Name("CallKind::Import"),
            "`CallKind::ImportPreview` is new: interceptors now also see the preview of an import (not audited), so a `match` on `CallKind` needs an arm for it and a check on `CallKind::Import` may need the same rule for previews",
        ),
        Change::Review(
            Pattern::Impl("RateStore"),
            "`RateStore::refund` is new: upload quotas give back what a file did not use. The default gives nothing back, which only makes the limit stricter; implement it to return the units",
        ),
        Change::Review(
            Pattern::Impl("DocStore"),
            "`DocStore::allow(ctx, key, write)` is new: it decides per document who may open and change it. The default keeps the old behavior (the kind's permission opens every document of the tenant), so implement it when documents belong to people",
        ),
        Change::Review(
            Pattern::Name("PageCheck"),
            "`PageCheck::problem` takes a `zeyn::Problem` (`Problem::new(code, summary).why(..).fix(..).docs(..)`, codes `Z06xx`) instead of a message",
        ),
    ],
};
