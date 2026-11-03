# Controles y selectores

Los campos de formulario eligen un control según su tipo. `.control(M::f, Control::...)` lo cambia y `.lookup(M::f, action)` convierte un campo en una búsqueda sobre el servidor. Cada control es por debajo un elemento de formulario nativo, así que los valores se envían igual con o sin el runtime, y cada control sigue su patrón ARIA para quienes usan el teclado y lectores de pantalla.

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## Controles por defecto

| Tipo de campo | Control |
|---|---|
| `String` | Entrada de texto; un área de texto que crece con un contador de caracteres cuando `max_len` supera 255 |
| `i32`, `i64` | Entrada numérica con las reglas `min` y `max` |
| `Money` | Importe en el formato numérico del usuario, con la moneda |
| `Decimal` | Número exacto en el formato del usuario, con el `scale` del campo (consulta [Decimal input](#decimal-and-percent)) |
| `bool` | Casilla de verificación |
| Enum `#[choice]` | Select: un botón que abre una lista con búsqueda |
| `Vec` de un enum `#[choice]` | Multi select: chips y una lista donde se eligen varias opciones |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | Selectores de fecha y hora |
| `Vec<M>` de un modelo | Editor de líneas (consulta [Editor de líneas](lines.md)) |

## Otros controles

| Control | Para |
|---|---|
| `Control::Radio` | Una elección con pocas opciones, todas visibles |
| `Control::Segmented` | Una elección con dos a cinco opciones cortas |
| `Control::Listbox` | Una lista de elección larga con búsqueda al escribir |
| `Control::Switch` | Un `bool` que surte efecto como un ajuste |
| `Control::Percent` | Un `Decimal` o número entero mostrado con un signo de porcentaje |
| `Control::Password`, `Control::NewPassword` | Un campo de texto de tipo contraseña, con un botón para mostrar y ocultar |
| `Control::PasswordMeter` | Una contraseña nueva con un medidor de fortaleza |

## Lookup

Un lookup busca registros en el servidor mientras el usuario escribe. La acción recibe un `LookupQuery` y devuelve `Page<LookupItem>`; el campo guarda el id.

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)` hace lo mismo para un campo `Vec<i64>` y muestra los elegidos como etiquetas que se pueden quitar.

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` muestra varias columnas en la lista. La búsqueda sigue yendo a la acción en el servidor, con su permiso comprobado en la búsqueda de cada pulsación de tecla; cada elemento da sus valores con `LookupItem::cells`, en el orden de las cabeceras. `LookupQuery::apply` busca también en las celdas. Los lectores de pantalla oyen cada opción como "Code: C-2, Name: Mert Kaya, City: Ankara". En pantallas estrechas las columnas se apilan dentro de cada opción.

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## Multi select

Un `Vec` de un enum `#[choice]` se edita con un multi select: las opciones elegidas se muestran como chips con un botón para quitarlas, y un combobox abre una lista donde las opciones se activan y desactivan (listbox APG con `aria-multiselectable`, dentro de un combobox). Por debajo hay un `<select multiple>` nativo, así que el campo funciona sin el runtime. El runtime envía los valores unidos por comas (`Email,Sms`); el servidor los lee uno a uno, rechaza los valores desconocidos y descarta los repetidos. En JSON (APIs, herramientas de IA, la base de datos) el campo es un array de los valores de las opciones.

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| Tecla | Qué hace |
|---|---|
| Flecha abajo, Flecha arriba, Enter, Espacio | Abre la lista en el combobox cerrado |
| Inicio, Fin | Abre la lista en la primera o la última opción |
| Flecha abajo, Flecha arriba | Se mueve por la lista abierta |
| Mayús con Flecha abajo o Flecha arriba | Se mueve y activa o desactiva la opción siguiente |
| Espacio, Enter | Activa o desactiva la opción activa; la lista sigue abierta |
| Letras | Saltan a la siguiente opción que empieza por ellas |
| Retroceso, Suprimir | En el combobox cerrado: quita la última opción elegida |
| Escape, Alt con Flecha arriba | Cierra la lista; el foco se queda en el combobox |
| Tab | Cierra la lista y avanza |

Ctrl+A (seleccionar todo en el patrón APG) se deja al navegador, como exige la política de atajos; los botones para quitar de los chips están en el orden de tabulación. La lista, los chips y las columnas del lookup necesitan unos 1,7 KB de script con gzip y ninguna petición; el campo son 3 nodos más uno por cada opción elegida.

## Decimal y porcentaje

Los campos `Decimal` guardan números exactos: ninguna coma flotante entre la entrada, el servidor, la base de datos y la exportación. La entrada da formato al número en el idioma del usuario mientras se escribe (`1.234,5678` en turco, `1,234.5678` en inglés) y permite el `scale` de decimales del campo; el servidor lo lee en el mismo idioma y vuelve a comprobar `scale`, `precision`, `min` y `max`.

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent` mantiene el valor en puntos porcentuales: `12.5` se muestra como `12.5 %` en inglés y `%12,5` en turco y se guarda como `12.5`. Los lectores de pantalla oyen la palabra "percent" a través de la descripción de la entrada.

## Entrada con máscara

Un campo de texto con la regla `mask` muestra su patrón mientras el usuario escribe y envía solo los caracteres sin formato. El patrón es la regla: el servidor lee quitando los literales escritos (`(532) 123 45 67` se convierte en `5321234567`), pasa a mayúsculas los huecos `A` y rechaza un valor que no llene todos los huecos. Las llamadas a la API y las herramientas de IA envían el valor sin formato y reciben la misma comprobación.

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| Carácter del patrón | Significa |
|---|---|
| `9` | Un dígito |
| `a` | Una letra |
| `A` | Una letra, escrita en mayúscula |
| `*` | Una letra o un dígito |
| una barra invertida | El carácter siguiente es un literal, como `\T` |
| cualquier otra cosa | Un literal: se muestra, nunca se guarda |

Sin el runtime, el campo es una entrada de texto simple con el valor sin formato; los usuarios pueden escribirlo con o sin los literales.

## Contraseña

`Control::Password` (inicio de sesión), `Control::NewPassword` (registro, cambio) y `Control::PasswordMeter` (una contraseña nueva con un medidor de fortaleza) son entradas de contraseña que nunca se rellenan desde el servidor y nunca se guardan en borradores. Un botón para mostrar y ocultar sigue el patrón APG de botón de alternancia: su nombre sigue siendo "Show password" y `aria-pressed` indica si el texto se muestra. El formulario vuelve a ocultar la contraseña cuando se envía.

El medidor es una pista calculada en el navegador a partir de la longitud, los tipos de caracteres y los patrones evidentes, mostrada como cuatro barras y un texto ("Strength: good") en una región en vivo educada. Lo que acepta el servidor lo deciden las reglas del campo (`min_len`, una función `validate` del modelo), nunca el medidor.

@live inputs

### Teclado

| Tecla | Dónde | Qué hace |
|---|---|---|
| Flecha arriba, Flecha abajo | entradas decimales y de porcentaje | Avanza una unidad entera dentro de `min` y `max` |
| Re Pág, Av Pág | entradas decimales y de porcentaje | Avanza de diez en diez |
| Retroceso | entrada con máscara | Borra el carácter anterior al cursor; sobre un literal borra el carácter anterior al literal |
| Tab | entrada de contraseña | Pasa al botón de mostrar y ocultar |
| Enter, Espacio | botón de mostrar y ocultar | Muestra u oculta la contraseña; el foco se queda en el botón |

### Presupuesto y comprobaciones

| | |
|---|---|
| Script añadido | unos 2 KB con gzip (7 KB sin comprimir) en `components.js` para las máscaras, la alternancia y el medidor; las entradas decimales y de porcentaje usan el control numérico |
| Nodos DOM | 3 para una entrada con máscara, 4 para una contraseña con su botón, 11 con el medidor |
| Peticiones | ninguna mientras se escribe |
| Comprobaciones del servidor | `scale`, `precision`, `min`, `max` y `mask` se ejecutan en cada guardado, desde la página, la API y las herramientas de IA por igual |

## Selectores de fecha y hora

Los selectores aceptan entrada escrita en el formato del usuario y abren un calendario o una lista de horas. `DateOptions` fija límites: `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`. `.range(from, to)` une dos campos de fecha en un selector de rango con preajustes como *This month*. El ejemplo en vivo está en [Hora e idiomas](time-and-languages.md#pickers).
