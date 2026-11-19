fn screen() -> Screen {
    Screen::new()
        /* the order of arguments flips */
        .bar("second").baz(first(1, 2))
        .keep(3)
        .foo(
            a, // first
            b,
        )
        .last()
}

fn nested() { x.bar(3).baz(y.bar(2).baz(1)); }
