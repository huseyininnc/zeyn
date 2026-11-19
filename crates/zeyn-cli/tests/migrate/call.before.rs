fn screen() -> Screen {
    Screen::new()
        /* the order of arguments flips */
        .foo(first(1, 2),   "second")
        .keep(3)
        .foo(
            a, // first
            b,
        )
        .gone()
        .last()
}

fn nested() { x.foo(y.foo(1, 2), 3); }
