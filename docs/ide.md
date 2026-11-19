# Editor support and macro error spans

Zeyn apps are written by AI agents and read by people, so a mistake inside `#[entity]`,
`#[model]`, `#[choice]`, `#[params]`, `#[action]`, `#[screen]` or `t!` has to be reported on the
token that is wrong, with a message that says what to write instead. This page records how that
was measured and what the macros do so an editor keeps working after a mistake.

## What the macros do after a mistake

- The error is built with the span of the offending token (`syn::Error::new` on the token's
  span, or `new_spanned` when the mistake is a whole item such as `<T>`). A type check that the
  compiler must report (a return type that is not a screen, a field type with no `FieldType`) is
  emitted with the span of the type.
- The item is emitted again after the error, so one mistake does not hide the function or struct
  from the rest of the crate. Helper attributes (`#[rule]`, `#[key]`, `#[label]`, `#[mask]`,
  `#[readonly]`, `#[version]`, `#[sensitive]`, and `#[label]` and `#[default]` on a `#[choice]`
  variant) are taken off that copy, so they do not add a `cannot find attribute` error for every
  other field.
- `t!` expands to `compile_error!` followed by `unreachable!()` when its key is wrong. The
  `!` type fits any place the text is used, so the expression around it keeps its type.
- Generated code uses `::zeyn::` paths only.

## How it was measured

A fixture crate depends on `zeyn` by path. Each case is one small `lib.rs` with one mistake and
the token that is wrong. `cargo check --message-format=json` gives the primary span of the first
error, and the span text is compared with the expected token. The error count is every
`error` level message of the run.

The cases below are the mistakes an agent makes most often. Two more were checked as controls:
an action without a return type and an action that returns a type that cannot be serialized both
compile (the first returns `()`; the second fails only where the result is used), and a valid
`t!` key gives no error.

Completion inside the macro body could not be measured here: `rust-analyzer` was not installed
in the environment that produced the numbers, and its headless commands report diagnostics, not
completion. The tolerance rules above are what rust-analyzer needs to keep completing the rest
of the file; re-measure with `rust-analyzer diagnostics .` on the same fixtures when the
component is available. Known gap: after an `#[entity]` error the generated key type (`ProductId`)
does not exist, so uses of it are unresolved in an editor until the error is fixed.

## Result

Spans on the offending token: 34 of 40 cases before, 39 of 40 after. Errors reported for the
same cases: 58 before, 49 after. A span that is not on the token is in bold.

| Case | Message | Span before | Span after | Errors |
|---|---|---|---|---|
| action: no access | action `delete_product` has no permission | `delete_product` | `delete_product` | 1 to 1 |
| action: typo permission key | unknown action option | `permision` | `permision` | 1 to 1 |
| action: permission not a string | expected string literal | `5` | `5` | 1 to 1 |
| action: invalid permission text | invalid permission `Stock` | `"Stock"` | `"Stock"` | 1 to 1 |
| action: public and permission | action `a` cannot be both `public` and have a permission | `"a.b"` | `"a.b"` | 1 to 1 |
| action: unknown flag | unknown action option | `audot` | `audot` | 1 to 1 |
| action: generic function | actions cannot be generic | **`<`** | `<T>` | 1 to 1 |
| action: injected arg is not injectable | `std::string::String` cannot be injected | `String` | `String` | 2 to 2 |
| action: confirm on screen | `confirm` belongs on the action that writes | `confirm` | `confirm` | 1 to 1 |
| screen: typo option | unknown screen option | `rout` | `rout` | 1 to 1 |
| screen: route unknown param | no field `idd` on type `&OrderParams` | `"/orders/{idd}"` | `"/orders/{idd}"` | 1 to 1 |
| screen: partial route segment | route segment `{id}-x` must be a whole segment `{field}` that names a  | `"/orders/{id}-x"` | `"/orders/{id}-x"` | 1 to 1 |
| screen: route not a string | expected string literal | `5` | `5` | 1 to 1 |
| screen: cache without public | `cache` is only for public screens | `300` | `300` | 1 to 1 |
| screen: params type not #[params] | `Plain` is not a screen params type | `Plain` | `Plain` | 1 to 1 |
| screen: wrong return type | the trait bound `i64: IntoScreen` is not satisfied | **`#[screen(public)]`** | `i64` | 1 to 1 |
| model: unknown rule | unknown rule | `requird` | `requird` | 2 to 1 |
| model: max_len on bool | text rules (`min_len`, `max_len`, `email`) cannot be used on `bool` | `bool` | `bool` | 1 to 1 |
| model: max_len not a number | expected integer literal | `"5"` | `"5"` | 2 to 1 |
| model: typo field attribute | cannot find attribute `ruel` in this scope | `ruel` | `ruel` | 1 to 1 |
| model: field type without FieldType | the trait bound `std::fs::File: Clone` is not satisfied | **`pub a: std::fs::File`** | **`pub a: std::fs::File`** | 9 to 9 |
| model: key on model | `#[key]` is only allowed on entities | `a` | `a` | 2 to 1 |
| model: typo option | unknown model option | `valdate` | `valdate` | 1 to 1 |
| model: min on String | the `min` and `max` rules cannot be used on `std::string::String` | `String` | `String` | 1 to 1 |
| entity: typo table option | unknown entity option | `tabel` | `tabel` | 2 to 1 |
| entity: missing table | entity `P` has no table | `P` | `P` | 2 to 1 |
| entity: no key field | entity `P` needs exactly one `#[key]` field, found 0 | `P` | `P` | 1 to 1 |
| entity: two keys | entity `P` needs exactly one `#[key]` field, found 2 | **`P`** | `b` | 3 to 1 |
| entity: tuple struct | a entity must have named fields | `P` | `P` | 1 to 1 |
| entity: unknown mask option | unknown mask option | `last5` | `last5` | 2 to 1 |
| entity: scale out of range | `scale` is at most 28 | `40` | `40` | 2 to 1 |
| choice: no variants | a choice needs at least one variant | `E` | `E` | 1 to 1 |
| choice: on a struct | expected `enum` | `struct` | `struct` | 1 to 1 |
| choice: with options | `#[choice]` takes no options | `x` | `x` | 1 to 1 |
| params: with options | `#[params]` takes no options | `x` | `x` | 1 to 1 |
| params: on a function | `#[params]` can only be used on a struct or an enum | **`pub`** | `f` | 1 to 1 |
| t!: unknown key | t!: key `a.zzz` is missing in locales/en.json | `"a.zzz"` | `"a.zzz"` | 1 to 1 |
| t!: missing argument | t!: `n` in locales/en.json uses {name} but no `name = ...` argument wa | `"n"` | `"n"` | 1 to 1 |
| t!: unknown argument | t!: argument `who` is not used by `a.b` in locales/en.json | **`"a.b"`** | `who` | 1 to 1 |
| t!: key not a literal | expected string literal | `KEY` | `KEY` | 1 to 1 |

The remaining miss is a field whose type is not a model field type (`std::fs::File`). The
custom message `cannot be used as a model field` points at the type, but rustc reports the
derives of the generated struct first, on the whole field, so the first of nine errors points at
`pub a: std::fs::File` and not at the type alone. The line is right; the column is the field.

## Compile fail tests

`crates/zeyn/src/compile_fail.rs` holds one `compile_fail` doctest per rule. A doctest only
proves that the code does not compile; the message text and the span are checked with the
fixture run described above.
