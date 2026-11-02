#set document(title: data.title)
#set page(
  paper: "a4",
  flipped: data.columns.len() > 5,
  margin: (x: 1.5cm, y: 1.6cm),
  footer: context [
    #set text(8pt, fill: luma(90))
    #data.title #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: "Libertinus Serif", size: 9pt, lang: data.lang)

#text(15pt, weight: "bold", data.title)
#if data.note != none [ #h(1em) #text(fill: luma(90), data.note) ]
#v(6pt)

#table(
  columns: data.columns.len(),
  stroke: 0.4pt + luma(190),
  inset: (x: 4pt, y: 3pt),
  align: (x, y) => if data.columns.at(x).numeric { right } else { left },
  fill: (x, y) => if y == 0 { luma(235) },
  table.header(..data.columns.map(c => text(weight: "bold", c.label))),
  ..data.rows.flatten(),
)
