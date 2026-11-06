# Código, licencia y seguridad

Zeyn es de código abierto. Esta página dice dónde está el código, bajo qué licencia se publica, cómo se prueba y cómo informar de un problema de seguridad.

## Código fuente

El código y las issues están en GitHub: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). Los crates se publican en [crates.io](https://crates.io/crates/zeyn) y su referencia de API está en [docs.rs](https://docs.rs/zeyn).

## Licencia

Zeyn tiene doble licencia: la [licencia MIT](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) o la [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE), a tu elección (`MIT OR Apache-2.0` en `Cargo.toml`). Las contribuciones se aceptan bajo las mismas dos licencias.

## Calidad

El repositorio tiene 767 funciones de prueba (`#[test]` y `#[tokio::test]` en los crates y los ejemplos, contadas el 2026-11-06), además de los ejemplos de los comentarios de documentación, que se ejecutan como doctests.

Cada pull request y cada push a `dev` y `main` ejecuta la CI:

- `cargo fmt --all --check` para el formato;
- `cargo clippy --workspace --all-targets -- -D warnings`, así que cualquier aviso de lint hace fallar la compilación;
- `cargo test --workspace`, las pruebas de cada crate y ejemplo;
- `cargo test -p zeyn` con las features `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email` y `accounts`;
- `cargo test -p zeyn-cli`, que además compila el framework sin su feature de servidor;
- `cargo deny` para licencias, avisos de seguridad y dependencias prohibidas.

Los resultados de cada ejecución son públicos en la [página de Actions](https://github.com/huseyininnc/zeyn/actions).

## Seguridad

- **Versiones con soporte.** Zeyn está en alpha. Solo la última versión `0.0.x` recibe correcciones de seguridad.
- **Cómo informar.** Informa de una vulnerabilidad en privado mediante GitHub Security Advisories: abre la pestaña Security del repositorio y elige "Report a vulnerability" ([enlace directo](https://github.com/huseyininnc/zeyn/security/advisories/new)). Nunca lo hagas en una issue, una discusión o un pull request públicos.
- **Respuesta.** Se acusa recibo de cada informe en un plazo de 5 días hábiles. Cuando se publica la corrección, se publica un aviso de seguridad y se reconoce a quien informó, salvo que prefiera no aparecer.

La política completa, con lo que debe incluir un informe y lo que cubre, está en [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md).

## Más

- [Registro de cambios](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): los cambios de cada versión.
- [Hoja de ruta](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): la próxima versión y lo planificado.
- [Cómo contribuir](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): cómo compilar, probar y enviar un cambio.
- [Código de conducta](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): cómo trabajamos juntos.
- [Soporte](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): dónde hacer preguntas e informar de errores.
