# Apps and shared libraries in separate repositories

Zeyn works in a single repository, but many teams keep each app in its own repository and put
shared code (entities, services, whole features) into libraries that several apps use. This guide
shows how to set that up, and the one rule that keeps it working.

## What goes where

| Repository | Created with | Contains |
|---|---|---|
| App | `zeyn new billing` | `main.rs`, `app.rs`, app features, config, app catalogs |
| Shared library | `zeyn new acme-domain --lib` | entities, services, features, their catalogs, `CATALOGS` |

A library feature is the same as an app feature: models, actions, screens, permissions and
translations live together in one folder. The app decides which library features it mounts.

```rust
let i18n = I18n::new("en")
    .catalogs(acme_domain::CATALOGS)?
    .catalog("en", include_str!("../locales/en.json"))?
    .catalog("tr", include_str!("../locales/tr.json"))?;

App::new(t!("app.name"))
    .i18n(i18n)
    .service(acme_domain::customers::CustomerStore::default())
    .feature(acme_domain::customers::feature())
```

Library catalogs are loaded first, so an app can override a library string by defining the same key.
Prefix library keys with the feature name (`customers.list`) so two libraries never collide.

## How apps get the library

| Option | Good | Watch out for |
|---|---|---|
| Git dependency with a tag | Nothing to run, works with any Git server | Cargo fetches the whole repository; every app lists the URL |
| Private Cargo registry (Kellnr, GitLab package registry, others) | Normal `version = "0.3"` dependencies, semver resolution | A service to run, back up and secure |
| One workspace repository for everything | One build, one `Cargo.lock`, refactors in one commit | Every team sees every app; access control is per repository, not per folder |

Start with Git tags. Move to a registry when you have more than a handful of libraries or when
version resolution by hand becomes tedious. The app code does not change; only `Cargo.toml` lines do.

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new` writes `zeyn = "<version>"` by default. Until Zeyn is published, change that line to the
Git form above in every new app and library.

## The rule: one Zeyn version everywhere

Every library and every app must depend on exactly the same Zeyn. With Git dependencies that means
the same URL and the same tag.

Cargo treats two different Zeyn sources as two different crates. A library built against tag
`v0.0.2` returns a `zeyn::Feature` that an app built against `v0.0.3` cannot accept, and the compiler
reports a type mismatch between two things with the same name. A registry relaxes this to "the same
semver compatible version", but the rule stays: one Zeyn per build.

To upgrade Zeyn:

1. Update the tag in each library, run its tests, release a new library tag.
2. Update Zeyn and the library tags in each app together, in one commit.

`cargo tree -d -i zeyn` lists duplicates. If it prints more than one Zeyn, a dependency still points
at an old tag.

## Step by step

1. Create the library and push it to its own repository.

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. Add it to the app's `Cargo.toml` with the tag, and mount its catalogs, services and features as
   shown above.

3. Allow the app's CI to read the library. The generated `.gitlab-ci.yml` rewrites
   `https://<gitlab host>/` and `ssh://git@<gitlab host>/` URLs to use the job token. In GitLab, open
   the library project, then CI/CD settings, Job token permissions, and add the app project to the
   allowlist.

4. Run `app.check()` in an app test (the generated `tests/app.rs` does). It fails when a mounted
   feature has structural problems, for example a translation key that one language is missing.

## Working on a library and an app at the same time

Point the app at your local checkout without editing `Cargo.toml`. Put this in the app's
`.cargo/config.toml` and keep that file out of Git:

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

Remove it, tag the library and bump the tag in the app when the change is done.

## Releasing a library

1. Bump `version` in `Cargo.toml`.
2. Run `cargo semver-checks` if it is installed. It reports public API changes that break callers.
   A breaking change needs a new minor version while the library is below 1.0.
3. Commit, tag `v<version>`, push the tag. The CI job `tag-matches-version` fails when the tag and
   `Cargo.toml` disagree.
