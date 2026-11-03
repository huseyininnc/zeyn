# Cuentas de usuario

Las aplicaciones que mantienen sus propios usuarios activan la feature de Cargo `accounts`. Incluye registro,
contraseñas, confirmación por correo electrónico, restablecimiento de contraseña, bloqueo, roles, inicio de sesión
en dos pasos, passkeys, inicios de sesión externos, pantallas listas para usar y endpoints JSON para aplicaciones móviles. Las aplicaciones que inician sesión a través de un
proxy de inicio de sesión único o de un proveedor de identidad no la necesitan. La referencia completa, con una tabla para
quienes conocen ASP.NET Core Identity, está en `docs/accounts.md`.

```toml
zeyn = { version = "0.0.6", features = ["accounts"] }
```

## Actívala

```rust
use zeyn::accounts::{self, Accounts, MailSender, MemoryAccountStore};

let accounts = Accounts::new(MemoryAccountStore::new(), config.secret("accounts.key")?)
    .public_url("https://shop.example.com")
    .sender(MailSender(mailer.clone()))
    .role("admins", [accounts::USERS_READ, accounts::USERS_WRITE, accounts::ROLES_WRITE])
    .role("clerks", ["shop.order.read", "shop.order.write"]);

App::new("Shop")
    .mailer(mailer)
    .accounts(accounts)
    .feature(accounts::ui().admin(true).build())
```

- La clave sale de la configuración, tiene al menos 32 bytes y nunca cambia.
- Usa `SqlAccountStore` con un `Db` para PostgreSQL u Oracle; `accounts::create_tables` proporciona la
  migración.
- Los roles se convierten en permisos mediante `role(...)`. Los claims de rol de tipo `permission` añaden más desde
  la pantalla de administración.

## Usa los managers en las acciones

```rust
use zeyn::accounts::{SignInManager, UserManager};

/// Sends the signed in user a new confirmation link.
#[action(public, ai = false)]
pub async fn welcome_again(_: (), users: UserManager, ctx: Ctx) -> Result<()> {
    let name = ctx.identity().user.clone().ok_or(Error::Forbidden)?;
    let user = users.find_by_name(&name).await?.ok_or(Error::NotFound)?;
    let _link_token = users.generate_email_confirmation_token(&user);
    Ok(())
}
```

## Campos propios en el usuario

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## Pantallas

`accounts::ui()` proporciona inicio de sesión, registro, recuperación de contraseña, confirmación por correo electrónico, inicio de sesión
en dos pasos, códigos de recuperación, bloqueo, las páginas de ajustes de la cuenta y, con `admin(true)`, usuarios y
roles. `registration(false)` oculta el registro; `without("account_passkeys")` omite una
pantalla para que la aplicación sirva la suya en la misma ruta. Cada paso entre ellas ocurre sin
cargar la página, y funcionan ambos temas y ambos idiomas.

## Reglas que conviene conocer

- El inicio de sesión responde igual para un usuario desconocido y para una contraseña incorrecta, y tarda lo mismo.
- Cinco contraseñas incorrectas bloquean la cuenta durante cinco minutos; los intentos también se limitan por dirección.
- Cambiar la contraseña cierra la sesión del usuario en otros dispositivos en un plazo de treinta minutos.
- Los hashes de contraseña, los stamps, las claves, los enlaces y los códigos nunca llegan a una pantalla, a una respuesta de la API, al registro ni
  a un modelo de IA.
- Los usuarios migrados desde una aplicación .NET conservan sus contraseñas; el primer inicio de sesión las vuelve a calcular con hash.
