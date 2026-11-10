//! CodeEditor examples: a report expression with completion of tables and fields and error
//! markers, an SQL query, and the same editor as a model form field through `Control::Code`.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{CodeEditor, CodeError, CodeQuery, Completion, CompletionKind};

/// The sample tables and their fields that completion offers.
const TABLES: [(&str, &[&str]); 2] = [
    ("Orders", &["Total", "Date", "Customer", "Discount"]),
    ("Customers", &["Name", "City", "Since"]),
];

/// The functions that completion offers.
const FUNCTIONS: [(&str, &str); 3] = [
    ("Round", "Round(value, digits)"),
    ("Sum", "Sum(values)"),
    ("IIf", "IIf(condition, then, else)"),
];

/// Suggests tables after `[`, their fields after `[Table.`, and functions elsewhere.
#[action(public)]
pub async fn complete_expression(q: CodeQuery) -> Result<Vec<Completion>> {
    let before = q.before();
    let word = q.word();
    let in_brackets = before.rfind('[') > before.rfind(']');
    let mut out = Vec::new();
    if in_brackets {
        match word.split_once('.') {
            Some((table, field)) => {
                for (t, fields) in TABLES {
                    if t.eq_ignore_ascii_case(table) {
                        out.extend(
                            fields
                                .iter()
                                .filter(|f| {
                                    f.to_lowercase()
                                        .starts_with(&field.to_lowercase())
                                })
                                .map(|f| {
                                    Completion::new(CompletionKind::Field, *f)
                                        .insert(format!("{t}.{f}]"))
                                        .detail(t)
                                }),
                        );
                    }
                }
            }
            None => out.extend(
                TABLES
                    .iter()
                    .filter(|(t, _)| {
                        t.to_lowercase()
                            .starts_with(&word.to_lowercase())
                    })
                    .map(|(t, fields)| {
                        Completion::new(CompletionKind::Table, *t)
                            .insert(format!("{t}."))
                            .detail(format!("{} fields", fields.len()))
                    }),
            ),
        }
    } else if !word.is_empty() {
        out.extend(
            FUNCTIONS
                .iter()
                .filter(|(f, _)| {
                    f.to_lowercase()
                        .starts_with(&word.to_lowercase())
                })
                .map(|(f, sig)| {
                    Completion::new(CompletionKind::Function, *f)
                        .insert(format!("{f}("))
                        .detail(*sig)
                }),
        );
    }
    Ok(out)
}

/// Marks every `[Table.Field]` reference that names no known table or field.
#[action(public)]
pub async fn check_expression(q: CodeQuery) -> Result<Vec<CodeError>> {
    let mut errors = Vec::new();
    for (n, line) in q.text.split('\n').enumerate() {
        let mut rest = line;
        let mut offset = 0;
        while let Some(open) = rest.find('[') {
            let Some(close) = rest[open..].find(']') else {
                break;
            };
            let name = &rest[open + 1..open + close];
            let known = name
                .split_once('.')
                .is_some_and(|(t, f)| {
                    TABLES
                        .iter()
                        .any(|(tt, fields)| *tt == t && fields.contains(&f))
                });
            if !known {
                let column = line[..offset + open].chars().count() as u32 + 1;
                errors.push(CodeError::new(
                    n as u32 + 1,
                    column,
                    name.chars().count() as u32 + 2,
                    format!("Unknown field [{name}]"),
                ));
            }
            offset += open + close + 1;
            rest = &rest[open + close + 1..];
        }
    }
    Ok(errors)
}

pub fn overview(_ctx: &Ctx) -> Node {
    CodeEditor::new("total", CodeLang::Expression)
        .label("Total with tax")
        .value("Round([Orders.Total] * 1.18m, 2)\n  - [Orders.Discont]")
        .rows(4)
        .complete(complete_expression)
        .check(check_expression)
        .into()
}

pub fn sql(_ctx: &Ctx) -> Node {
    CodeEditor::new("query", CodeLang::Sql)
        .label("Report query")
        .value("/* Open orders of the last week */\nSELECT o.number, c.name, o.total\nFROM orders o\nJOIN customers c ON c.id = o.customer_id\nWHERE o.state = 'open'\nORDER BY o.created_at DESC;")
        .into()
}

#[model]
pub struct CalcField {
    #[rule(required, max_len = 2000)]
    pub formula: String,
}

/// Shows the formula; the catalog stores nothing.
#[action(public)]
pub async fn save_calc_field(c: Valid<CalcField>) -> Result<Option<Effect>> {
    Ok(Some(Effect::Toast {
        message: Text::from(format!("Saved: {}", c.into_inner().formula)),
    }))
}

pub fn form(_ctx: &Ctx) -> Node {
    Form::of(CalcField {
        formula: "[Customers.City] + \" \" + [Customers.Name]".into(),
    })
    .field(CalcField::formula)
    .control(CalcField::formula, Control::Code(CodeLang::Expression))
    .complete(CalcField::formula, complete_expression)
    .check(CalcField::formula, check_expression)
    .submit("Save", save_calc_field)
    .into()
}

/// What the catalog shows of `CodeEditor`.
pub const ENTRY: Entry = Entry {
    name: "CodeEditor",
    docs: "components/controls#code-editor",
    api: &["CodeEditor", "CodeQuery", "Completion", "CodeError"],
    overview: &[ex!(overview)],
    examples: &[ex!(sql), ex!(form)],
};
