# Roadmap

Zeyn is in alpha. This page lists what is released, what the next release holds and what is planned. It makes no promise about dates; plans change as the framework is used and reviewed.

## Released

Every release and its changes are in [CHANGELOG.md](CHANGELOG.md). The latest release is on [crates.io](https://crates.io/crates/zeyn).

## Next release

The changes waiting for the next release are under `[Unreleased]` in [CHANGELOG.md](CHANGELOG.md). In short:

- Topics: components refresh in place when an action, a filter or the realtime hub names their topic.
- Faster long lists and card grids: virtual lists, image placeholders, low resolution images first, and card grids that keep only the rows near the viewport.
- Design system v2: new type scale, colors, shapes, icons, buttons, forms, grids, charts, KPI cards, states and shell, with a compact density mode.
- An app top bar (`App::header`) with breadcrumb, search and account parts, a sign in shell (`Shell::auth()`) and app logos.
- Generators and checks: `zeyn g screen`, a folder per part of a feature, file and function size limits in new projects.
- Security: separate read and write access for file stores, and password fields kept out of the audit trail.

## Planned

- Component sets for chat interfaces and social feeds.
- Faster card grids on slow phones.
- The remaining parts of the design system.
- A testing guide, with helpers for testing screens and actions.
- An extension model for third party components.
- Agent tooling:
  - the documentation served by `zeyn mcp`;
  - `zeyn check --json` with stable error codes;
  - more checks in `zeyn check`;
  - a public benchmark of agents writing Zeyn apps.

## How to propose

Ideas and questions go to GitHub Discussions once they are enabled for the repository. Until then, and for concrete requests, open an issue with the feature request form (https://github.com/huseyininnc/zeyn/issues). [CONTRIBUTING.md](CONTRIBUTING.md) explains how to send a change.
