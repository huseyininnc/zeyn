#set document(title: data.title)
#set page(paper: "a4", margin: 2cm)
#set text(font: "Libertinus Serif", size: 10pt, lang: data.lang)

#grid(
  columns: (1fr, auto),
  [
    #text(18pt, weight: "bold", data.seller)
    #linebreak()
    #text(fill: luma(90), data.seller_address)
  ],
  align(right)[
    #text(16pt, weight: "bold", data.title)
    #linebreak()
    #data.number_label: #data.number
    #linebreak()
    #data.date_label: #data.date
  ],
)

#v(12pt)
#block(inset: 8pt, stroke: 0.5pt + luma(190), radius: 3pt, width: 50%)[
  #text(fill: luma(90), data.customer_label)
  #linebreak()
  #text(weight: "bold", data.customer)
  #linebreak()
  #data.city
]

#v(12pt)
#table(
  columns: (auto, 1fr, auto, auto, auto),
  stroke: (x, y) => if y == 0 { (bottom: 0.8pt) } else { (bottom: 0.3pt + luma(200)) },
  inset: 6pt,
  align: (x, y) => if x >= 2 { right } else { left },
  table.header(..data.columns.map(c => text(weight: "bold", c))),
  ..data.lines.map(l => (l.code, l.product, l.quantity, l.price, l.amount)).flatten(),
)

#align(right)[
  #v(6pt)
  #text(12pt, weight: "bold")[#data.total_label: #data.total]
]
