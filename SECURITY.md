# Security policy

## Supported versions

Zeyn is in alpha. Only the latest `0.0.x` release gets security fixes. Older releases do not.

## Reporting a vulnerability

Report vulnerabilities privately through GitHub private vulnerability reporting: open the Security tab of https://github.com/huseyininnc/zeyn and choose "Report a vulnerability" (direct link: https://github.com/huseyininnc/zeyn/security/advisories/new).

Do not report security problems in public issues, discussions or pull requests.

Please include:

- the affected crate and version (`zeyn`, `zeyn-cli`, `zeyn-macros`, `zeyn-island` or `zeyn-desktop`), and enabled Cargo features if they matter
- a description of the problem and its impact
- steps to reproduce, ideally a minimal app or test
- the operating system and Rust version
- a suggested fix, if you have one

## What to expect

- We acknowledge a report within 5 working days.
- We keep you informed while we confirm the problem and work on a fix.
- After a fix is released, we publish a GitHub security advisory and credit you, unless you prefer not to be named.
- Please keep the details private until the fix is released.

## Scope

In scope: the crates in this repository and the official sites (https://zeynsoft.com, https://docs.zeynsoft.com, https://framework.zeynsoft.com, https://components.zeynsoft.com).

Out of scope: applications built with Zeyn by third parties, and vulnerabilities in dependencies that do not affect Zeyn (report those to the dependency).
