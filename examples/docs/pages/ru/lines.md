# Редактор строк

Поле `Vec<M>` модели это список строк, например строки счёта. `.lines(M::f, ...)` редактирует их как небольшую таблицу внутри формы: добавление, удаление и перемещение строк, переход между ячейками с клавиатуры и итоги, которые обновляются по мере ввода. Вся форма, включая строки, проверяется и сохраняется одним действием (action).

```rust
#[model]
pub struct Line {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<Line>,
}

Form::of(invoice)
    .field(Invoice::customer)
    .lines(Invoice::lines, |l| {
        l.column(Line::item)
            .column(Line::quantity)
            .column(Line::unit_price)
            .total(Line::quantity, Line::unit_price)
    })
    .submit(t!("docs.live.save"), save_invoice)
```

@live lines

## Колонки и итоги

`.column(Line::f)` добавляет редактируемую колонку с элементом управления, подходящим её типу. `.total(quantity, price)` показывает для каждой строки количество, умноженное на цену, а под строками сумму, в валюте формы.

## Проверка

Каждая строка проверяется правилами своей модели, а ошибки показываются в ячейке. `#[rule(required)]` на поле `Vec` означает как минимум одну строку. Число строк ограничивает сервер (`MAX_LINES`), что бы ни прислал клиент.

## Сохранение

Действие принимает `Valid<Invoice>`, поэтому каждая строка проходит свои правила до запуска действия:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
