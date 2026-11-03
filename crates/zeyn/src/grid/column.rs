//! [`DataGridColumn`]: the builder of the last added column of a [`DataGrid`], with every
//! `DataGrid` method passed through so a grid reads as one chain.

use super::*;

/// Configures the last added column. DataGrid methods can be called from here too.
pub struct DataGridColumn<E: Model, T> {
    pub(super) grid: DataGrid<E>,
    pub(super) field: Field<E, T>,
    pub(super) _type: PhantomData<fn() -> T>,
}

impl<E: Model, T> DataGridColumn<E, T> {
    fn last(&mut self) -> &mut Column<E> {
        self.grid
            .columns
            .last_mut()
            .expect("DataGridColumn always has a column")
    }

    /// Fixed width in pixels.
    pub fn width(mut self, px: u32) -> Self {
        self.last().width = Some(px);
        self
    }

    /// Takes a share of the remaining width.
    pub fn flex(mut self, weight: u32) -> Self {
        self.last().flex = Some(weight);
        self
    }

    /// Adds a filter box to the column.
    pub fn filter(mut self) -> Self {
        self.last().filter = true;
        self
    }

    /// The column cannot be sorted: the header has no sort button, the column menu no sort
    /// entries, and the server drops sort keys on it before the action runs. Use it when the
    /// action cannot order by the field (generated rows, a column the database has no index for),
    /// so the page never offers a sort the data does not follow.
    pub fn no_sort(mut self) -> Self {
        self.last().sort = false;
        self
    }

    fn aggregate(mut self, op: Aggregate) -> Self {
        let column = self.last();
        if !column.aggregates.contains(&op) {
            column.aggregates.push(op);
        }
        self
    }

    /// Shows the column total in the footer, computed on the server over every matching row.
    /// Compiles only on numeric and money columns.
    pub fn sum(self) -> Self
    where
        T: SupportsSum,
    {
        self.aggregate(Aggregate::Sum)
    }

    /// Shows the column average in the footer. Compiles only on numeric and money columns.
    pub fn avg(self) -> Self
    where
        T: SupportsSum,
    {
        self.aggregate(Aggregate::Avg)
    }

    /// Shows the smallest value in the footer. Compiles on numbers, money, dates and times.
    pub fn min(self) -> Self
    where
        T: SupportsMinMaxTotal,
    {
        self.aggregate(Aggregate::Min)
    }

    /// Shows the largest value in the footer. Compiles on numbers, money, dates and times.
    pub fn max(self) -> Self
    where
        T: SupportsMinMaxTotal,
    {
        self.aggregate(Aggregate::Max)
    }

    /// Keeps the column at the start edge (left in left to right languages) while the grid
    /// scrolls sideways. At most three columns per side stay pinned.
    pub fn pin_start(mut self) -> Self {
        self.last().pin = Some(Pin::Start);
        self
    }

    /// Keeps the column at the end edge while the grid scrolls sideways.
    pub fn pin_end(mut self) -> Self {
        self.last().pin = Some(Pin::End);
        self
    }

    /// Leaves the column out until the user shows it from the column menu.
    pub fn hidden(mut self) -> Self {
        self.last().hidden = true;
        self
    }

    /// Lets users edit this column in place when the grid has a save action (`DataGrid::edit_with`).
    /// Keys, read only fields and row versions never become editable; the server refuses
    /// values for them.
    pub fn editable(mut self) -> Self {
        self.last().editable = true;
        self
    }

    /// Draws the cells with a custom renderer. See [`CellRenderer`].
    pub fn cell(mut self, renderer: impl CellRenderer<T>) -> Self
    where
        T: 'static,
    {
        let field = self.field;
        self.last().cell = Some(Box::new(move |row, r| renderer.render(field.get(row), r)));
        self
    }

    /// Shows the choice as a colored badge; `tone` picks the color per option:
    /// `.badge(|u| match u { Unit::Kg => Tone::Info, _ => Tone::Neutral })`. Compiles only on
    /// `#[choice]` columns.
    pub fn badge(self, tone: impl Fn(&T) -> Tone + Send + Sync + 'static) -> Self
    where
        T: ChoiceType + FieldType + 'static,
    {
        let kind = self.field.meta().kind;
        self.cell(move |v: &T, r: &Render| {
            format!(
                r#"<span class="z-badge z-tone-{}">{}</span>"#,
                tone(v).code(),
                r.value(&v.to_value(), &kind)
            )
        })
    }

    /// Shows a number as a bar filled up to `max`, with the number beside it. Compiles only on
    /// numeric and money columns.
    pub fn progress(self, max: i64) -> Self
    where
        T: SupportsSum + FieldType + 'static,
    {
        let kind = self.field.meta().kind;
        self.cell(move |v: &T, r: &Render| {
            let value = v.to_value();
            let amount = match value {
                Value::Int(n) => n as f64,
                Value::Money(m) => m.minor as f64 / 100.0,
                _ => return String::new(),
            };
            let pct = if max > 0 { (amount / max as f64 * 100.0).clamp(0.0, 100.0) } else { 0.0 };
            format!(
                r#"<span class="z-progress"><span class="z-progress-track" aria-hidden="true"><span class="z-progress-bar" style="--z-pct:{pct:.1}%"></span></span><span class="z-progress-text">{}</span></span>"#,
                r.value(&value, &kind)
            )
        })
    }

    /// Makes the cell a link that runs an effect built from the row, such as opening a window.
    pub fn link(mut self, effect: impl Fn(&E) -> Effect + Send + Sync + 'static) -> Self
    where
        T: FieldType + 'static,
    {
        let field = self.field;
        let kind = field.meta().kind;
        self.last().cell = Some(Box::new(move |row, r| {
            format!(
                r#"<button type="button" class="z-link" tabindex="-1" data-zeyn-effect="{}">{}</button>"#,
                attr(&effect(row)),
                r.value(&field.get(row).to_value(), &kind)
            )
        }));
        self
    }

    /// Shows yes as a check mark and no as a dash, with the words for screen readers. Compiles
    /// only on yes/no columns.
    pub fn check_icon(self) -> Self
    where
        T: IsBool + FieldType + 'static,
    {
        self.cell(|v: &T, r: &Render| match v.to_value() {
            Value::Bool(b) => format!(
                r#"<span class="z-bool z-bool-{}" role="img" aria-label="{}"></span>"#,
                if b { "yes" } else { "no" },
                r.text(&Text::key(if b { "zeyn.yes" } else { "zeyn.no" }))
            ),
            _ => String::new(),
        })
    }

    /// Shows a moment as relative time ("3 minutes ago") in the user's language, with the exact
    /// time in the user's time zone as a tooltip. Compiles only on moment columns.
    pub fn relative_time(self) -> Self
    where
        T: IsMoment + FieldType + 'static,
    {
        self.cell(|v: &T, r: &Render| match v.to_value() {
            Value::DateTime(d) => {
                let exact = d.format(r.tz, r.locale);
                format!(r#"<time datetime="{d}" title="{exact}" data-zeyn-relative>{exact}</time>"#)
            }
            _ => String::new(),
        })
    }

    /// Shows how many matching rows have a value in this column. Works on every column.
    pub fn count(self) -> Self {
        self.aggregate(Aggregate::Count)
    }

    pub fn column<U: FieldType + 'static>(self, field: Field<E, U>) -> DataGridColumn<E, U> {
        self.grid.column(field)
    }

    pub fn mode(self, mode: DataMode) -> DataGrid<E> {
        self.grid.mode(mode)
    }

    pub fn paging(self, paging: Paging) -> DataGrid<E> {
        self.grid.paging(paging)
    }

    pub fn row_style(
        self,
        style: impl Fn(&E) -> Option<RowStyle> + Send + Sync + 'static,
    ) -> DataGrid<E> {
        self.grid.row_style(style)
    }

    pub fn on_open(self, open: impl Fn(&E) -> Effect + Send + Sync + 'static) -> DataGrid<E> {
        self.grid.on_open(open)
    }

    pub fn row_action<A>(
        self,
        label: impl Into<Text>,
        action: A,
        input: impl Fn(&E) -> A::Input + Send + Sync + 'static,
    ) -> DataGrid<E>
    where
        A: Action,
        A::Output: Outcome,
    {
        self.grid
            .row_action(label, action, input)
    }

    pub fn selectable(self) -> DataGrid<E> {
        self.grid.selectable()
    }

    pub fn row_menu(
        self,
        menu: impl Fn(&E) -> crate::menu::ContextMenu + Send + Sync + 'static,
    ) -> DataGrid<E> {
        self.grid.row_menu(menu)
    }

    pub fn exportable(self) -> DataGrid<E> {
        self.grid.exportable()
    }

    pub fn edit_with<A>(self, save: A) -> DataGrid<E>
    where
        A: crate::form::FormAction<E> + Action<Input = Valid<E>>,
        A::Output: Outcome,
    {
        self.grid.edit_with(save)
    }

    pub fn bulk_action<A, K>(
        self,
        label: impl Into<Text>,
        action: A,
        key: Field<E, K>,
    ) -> DataGrid<E>
    where
        A: BulkAction<K> + Action<Input = Vec<K>>,
        A::Output: Outcome,
        K: serde::de::DeserializeOwned + Send + 'static,
    {
        self.grid
            .bulk_action(label, action, key)
    }

    pub fn detail<S: ScreenDef>(
        self,
        detail: impl Fn(&E) -> (S, S::Params) + Send + Sync + 'static,
    ) -> DataGrid<E> {
        self.grid.detail(detail)
    }

    pub fn scope<U: FieldType>(self, field: Field<E, U>, value: U) -> DataGrid<E> {
        self.grid.scope(field, value)
    }

    pub fn sort_by<U: FieldType>(self, field: Field<E, U>, desc: bool) -> DataGrid<E> {
        self.grid.sort_by(field, desc)
    }

    pub fn group_by<U: FieldType>(self, field: Field<E, U>) -> DataGrid<E> {
        self.grid.group_by(field)
    }

    pub fn tree<K: FieldType>(self, parent: Field<E, Option<K>>) -> DataGrid<E>
    where
        Option<K>: FieldType,
    {
        self.grid.tree(parent)
    }

    /// Returns the grid.
    pub fn done(self) -> DataGrid<E> {
        self.grid
    }
}

impl<E: Model, T> From<DataGridColumn<E, T>> for DataGrid<E> {
    fn from(c: DataGridColumn<E, T>) -> Self {
        c.grid
    }
}

impl<E: Model, T> From<DataGridColumn<E, T>> for Node {
    fn from(c: DataGridColumn<E, T>) -> Self {
        Node::new(c.grid)
    }
}
