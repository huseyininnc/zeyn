# Hora e idiomas

Las fechas y las horas son una fuente habitual de errores en las aplicaciones de negocio: un cumpleaños que se mueve un día cuando un usuario viaja, un turno que empieza una hora tarde tras un cambio de hora. Zeyn da a cada tipo de valor su propio tipo, de modo que el compilador los mantiene separados.

## Cuatro tipos

| Tipo | Es | Ejemplo |
|---|---|---|
| `Date` | Un día de calendario, sin zona | Un cumpleaños, la fecha de una factura |
| `Time` | Una hora de reloj, sin zona | Horario de apertura |
| `DateTime` | Un instante exacto, guardado en UTC | Cuándo se creó un registro |
| `LocalDateTime` | Una fecha y hora de reloj en un lugar | Una cita a las 14:00 en Estambul |

Nunca guardes un día de calendario como un instante. Un `Date` es el mismo día en todas las zonas; un `DateTime` es el mismo instante en todas partes y muestra una hora de reloj distinta en cada zona.

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

`DateTime::from_local` falla para una hora de reloj que no existe en la zona, como durante un cambio de hora de primavera, en lugar de adivinar.

## El reloj

Usa `ctx.now()` y `ctx.today()`, nunca el reloj del sistema. `ctx.today()` es hoy en la zona del usuario. Las pruebas fijan el reloj para cada petición con `App::fixed_now(moment)`.

## Zonas horarias

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| Política | Los instantes se muestran en |
|---|---|
| `TimePolicy::User { default }` | La zona que informa el navegador del usuario, o `default` hasta que lo haga |
| `TimePolicy::Organization(zone)` | Una zona para todos |

`.time_zone_picker()` añade un selector al menú para que los usuarios elijan una zona a mano. Un proxy de inicio de sesión único también puede enviar la zona en una cabecera (`ForwardAuth::time_zone`). Los filtros de tabla en columnas `DateTime` se interpretan en la zona del usuario.

## Selectores

Los campos de fecha, hora y fecha y hora tienen selectores pensados para el teclado. `DateOptions` limita lo que se puede elegir:

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## Elegir el idioma

El servidor elige el idioma a partir de la cookie `zeyn_locale`, después del `Accept-Language` del navegador y por último del valor por defecto de la aplicación. `<html lang>` siempre coincide. El selector de idioma del menú define la cookie y sustituye la página en el sitio, sin recarga.

Con `App::locale_paths()` el idioma pasa a formar parte de la URL: el idioma predeterminado se queda en `/orders` y los demás usan `/tr/orders` o `/ar/orders`. Solo decide la ruta, así que un enlace compartido se abre en el mismo idioma, y cada pantalla sigue teniendo una sola ruta y lee `ctx.locale()`. Los enlaces del menú conservan el idioma y el selector abre la misma página en el otro idioma sin recargar. El árabe y los demás idiomas que se escriben de derecha a izquierda reciben `<html dir="rtl">`. Indica a los buscadores los idiomas de una página pública con `Screen::alternate("tr", "/tr/orders")`.

## Números, dinero y fechas

Los valores se formatean en el servidor para el idioma del usuario: `1.234,56` en turco y `1,234.56` en inglés, las fechas como `29.03.2026` o `3/29/2026`, y el dinero con el símbolo de su moneda. Las entradas leen de vuelta los mismos formatos.

`Money` guarda unidades menores enteras y una moneda, así que las sumas nunca pierden un céntimo:

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
