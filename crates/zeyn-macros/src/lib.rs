//! Procedural macros for Zeyn Framework.
//!
//! Generated code always refers to `::zeyn`. Use these macros through the `zeyn` crate, not directly.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Fields, FnArg, Ident, Item, ItemEnum, ItemFn, ItemStruct, LitInt, LitStr, Path, ReturnType, Type};

/// Holds and validates access options: `permission = "..."` or `public`.
#[derive(Default)]
struct AccessArgs {
    permission: Option<LitStr>,
    public: bool,
}

impl AccessArgs {
    /// Turns the options into a `::zeyn::Access` expression. Missing access is a compile error.
    fn tokens(&self, kind: &str, name: &Ident) -> syn::Result<Ts> {
        match (&self.permission, self.public) {
            (Some(p), false) => {
                check_permission(p)?;
                Ok(quote!(::zeyn::Access::Permission(#p)))
            }
            (None, true) => Ok(quote!(::zeyn::Access::Public)),
            (Some(p), true) => {
                Err(syn::Error::new(p.span(), format!("{kind} `{name}` cannot be both `public` and have a permission")))
            }
            (None, false) => Err(syn::Error::new(
                name.span(),
                format!(
                    "{kind} `{name}` has no permission\n  = help: add `permission = \"...\"`, or `public` if this {kind} is meant to be open to everyone"
                ),
            )),
        }
    }
}

/// Checks that a permission is a lowercase dotted name such as `stock.product.read`.
fn check_permission(lit: &LitStr) -> syn::Result<()> {
    let value = lit.value();
    let parts: Vec<&str> = value.split('.').collect();
    let valid = parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
    if valid {
        Ok(())
    } else {
        Err(syn::Error::new(
            lit.span(),
            format!(
                "invalid permission `{value}`\n  = help: use lowercase dotted names such as \"stock.product.read\""
            ),
        ))
    }
}

/// Returns the type of the first argument (input or params) and the return type.
fn fn_types(f: &ItemFn, kind: &str) -> syn::Result<(Ts, Ts)> {
    if !f.sig.generics.params.is_empty() {
        return Err(syn::Error::new(f.sig.generics.span(), format!("{kind}s cannot be generic")));
    }
    let input = match f.sig.inputs.first() {
        None => quote!(()),
        Some(FnArg::Typed(pt)) => {
            let ty = &pt.ty;
            quote!(#ty)
        }
        Some(FnArg::Receiver(r)) => {
            return Err(syn::Error::new(r.span(), format!("{kind}s cannot take `self`")));
        }
    };
    let output = match &f.sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, ty) => quote!(#ty),
    };
    Ok((input, output))
}

/// AI policy options shared by actions and screens: `ai = "guidance"`, `ai = false`, `confirm`.
#[derive(Default)]
struct AiArgs {
    hidden: bool,
    hint: Option<LitStr>,
    confirm: Option<proc_macro2::Span>,
}

impl AiArgs {
    /// Reads the value of `ai = ...`: a string with guidance for models, or `false`.
    fn parse_ai(&mut self, meta: &syn::meta::ParseNestedMeta) -> syn::Result<()> {
        if self.hidden || self.hint.is_some() {
            return Err(meta.error("`ai` is given twice"));
        }
        let lit: syn::Lit = meta.value()?.parse()?;
        match lit {
            syn::Lit::Str(s) if s.value().trim().is_empty() => Err(syn::Error::new(
                s.span(),
                "`ai` guidance is empty\n  = help: write a sentence for the model, or use `ai = false` to hide it from models",
            )),
            syn::Lit::Str(s) => {
                self.hint = Some(s);
                Ok(())
            }
            syn::Lit::Bool(b) if !b.value => {
                self.hidden = true;
                Ok(())
            }
            syn::Lit::Bool(b) => Err(syn::Error::new(
                b.span(),
                "`ai = true` is the default\n  = help: remove it, write `ai = \"guidance for models\"`, or use `ai = false` to hide it from models",
            )),
            other => Err(syn::Error::new(other.span(), "`ai` takes a string with guidance for models, or `false`")),
        }
    }

    /// Turns the options into a `::zeyn::Ai` expression.
    fn tokens(&self, kind: &str) -> syn::Result<Ts> {
        if let (true, Some(span)) = (self.hidden, self.confirm) {
            return Err(syn::Error::new(
                span,
                format!(
                    "a {kind} with `ai = false` is never offered to models, so `confirm` has no effect\n  = help: remove `confirm` or `ai = false`"
                ),
            ));
        }
        let offered = !self.hidden;
        let hint = self.hint.as_ref().map(LitStr::value).unwrap_or_default();
        let confirm = self.confirm.is_some();
        Ok(quote!(::zeyn::Ai { offered: #offered, hint: #hint, confirm: #confirm }))
    }
}

/// First paragraph of the item's `///` doc comment, on one line. Tools for AI models use it as
/// their description.
fn doc_text(attrs: &[syn::Attribute]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for a in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let syn::Meta::NameValue(nv) = &a.meta else { continue };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value else { continue };
        let value = s.value();
        let line = value.trim();
        if line.is_empty() {
            if lines.is_empty() {
                continue;
            }
            break;
        }
        lines.push(line.to_string());
    }
    lines.join(" ")
}

/// Turns a function into a unit struct of the same name with a `run` method.
///
/// `fn search_products(q: DataQuery)` becomes `struct search_products;` and `search_products::run(q)`.
fn unit_and_run(f: &ItemFn) -> Ts {
    let ItemFn { attrs, vis, sig, block } = f;
    let name = &sig.ident;
    let docs: Vec<_> = attrs.iter().filter(|a| a.path().is_ident("doc")).collect();
    let mut run_sig = sig.clone();
    run_sig.ident = format_ident!("run");
    quote! {
        #(#docs)*
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Default)]
        #vis struct #name;

        impl #name {
            #(#attrs)*
            #vis #run_sig #block
        }
    }
}

/// Code that resolves every argument after the first through `Inject`, then calls `run`.
///
/// Returns the statements that bind the injected values, the call expression and the dependency list.
fn injected_call(f: &ItemFn, kind: &str) -> syn::Result<(Ts, Ts, Ts)> {
    let mut binds = Vec::new();
    let mut args = Vec::new();
    let mut deps = Vec::new();
    let mut first = true;
    for (i, arg) in f.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pt) = arg else { continue };
        if first {
            first = false;
            args.push(quote!(input));
            continue;
        }
        let ty = &pt.ty;
        if matches!(**ty, Type::Reference(_)) {
            return Err(syn::Error::new(
                ty.span(),
                format!(
                    "injected {kind} arguments must be owned types\n  = help: take `Ctx` or the service by value; they are cheap to clone"
                ),
            ));
        }
        let var = format_ident!("__zeyn_arg{}", i);
        binds.push(quote_spanned!(ty.span()=> let #var = <#ty as ::zeyn::Inject>::inject(&ctx)?;));
        args.push(quote!(#var));
        deps.push(quote_spanned!(ty.span()=> <#ty as ::zeyn::Inject>::dep()));
    }
    let await_ = f.sig.asyncness.map(|_| quote!(.await));
    let ignore = if f.sig.inputs.is_empty() { quote!(let _ = input;) } else { quote!() };
    let call = quote!({ #ignore Self::run(#(#args),*) #await_ });
    let deps = quote!(::std::vec::Vec::<::core::option::Option<::zeyn::Dep>>::from([#(#deps),*]).into_iter().flatten().collect());
    Ok((quote!(#(#binds)*), call, deps))
}

/// On error, emits the original item too, so one mistake does not cause a cascade of errors.
fn or_error(result: syn::Result<Ts>, original: Ts) -> TokenStream {
    match result {
        Ok(ts) => ts.into(),
        Err(e) => {
            let err = e.to_compile_error();
            quote!(#err #original).into()
        }
    }
}

/// Defines a server side action. Access must be declared.
///
/// The first argument is the input; every other argument is injected.
///
/// Options: `permission = "..."` or `public`, `audit`, `reauth`, and for AI models
/// `ai = "guidance"`, `ai = false` (never offered to a model) and `confirm` (a model may run it
/// only after the user confirms in the app). The first paragraph of the doc comment becomes the
/// description of the action's AI tools.
///
/// ```ignore
/// #[action(permission = "stock.product.write", audit)]
/// pub async fn save_product(product: Valid<Product>) -> Result<Product> { ... }
/// ```
#[proc_macro_attribute]
pub fn action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(item as ItemFn);
    let mut access = AccessArgs::default();
    let mut audit = false;
    let mut reauth = false;
    let mut ai = AiArgs::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("permission") {
            access.permission = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("public") {
            access.public = true;
        } else if meta.path.is_ident("audit") {
            audit = true;
        } else if meta.path.is_ident("reauth") {
            reauth = true;
        } else if meta.path.is_ident("ai") {
            ai.parse_ai(&meta)?;
        } else if meta.path.is_ident("confirm") {
            ai.confirm = Some(meta.path.span());
        } else {
            return Err(meta.error(
                "unknown action option\n  = help: expected `permission = \"...\"`, `public`, `audit`, `reauth`, `ai = \"...\"`, `ai = false` or `confirm`",
            ));
        }
        Ok(())
    });
    syn::parse_macro_input!(attr with parser);
    let original = quote!(#f);
    or_error(expand_action(&f, &access, (audit, reauth), &ai), original)
}

fn expand_action(f: &ItemFn, access: &AccessArgs, flags: (bool, bool), ai: &AiArgs) -> syn::Result<Ts> {
    let (audit, reauth) = flags;
    let (input, output) = fn_types(f, "action")?;
    let name = &f.sig.ident;
    let access = access.tokens("action", name)?;
    let ai = ai.tokens("action")?;
    let doc = doc_text(&f.attrs);
    let name_str = name.to_string();
    let base = unit_and_run(f);
    let (binds, call, deps) = injected_call(f, "action")?;
    Ok(quote! {
        #base

        impl ::zeyn::Action for #name {
            type Input = #input;
            type Output = #output;
            const META: ::zeyn::ActionMeta = ::zeyn::ActionMeta {
                name: #name_str,
                access: #access,
                audit: #audit,
                reauth: #reauth,
                doc: #doc,
                ai: #ai,
            };

            fn deps() -> ::std::vec::Vec<::zeyn::Dep> {
                #deps
            }

            fn call(ctx: ::zeyn::Ctx, input: Self::Input) -> ::zeyn::BoxFuture<::zeyn::Result<Self::Output>> {
                ::std::boxed::Box::pin(async move {
                    #binds
                    ::core::result::Result::Ok(#call)
                })
            }
        }
    })
}

/// Defines a screen. The first argument is its params; a screen without params takes none or `_: ()`.
///
/// ```ignore
/// #[screen(route = "/products", permission = "stock.product.read")]
/// pub fn product_list() -> Screen { ... }
/// ```
#[proc_macro_attribute]
pub fn screen(attr: TokenStream, item: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(item as ItemFn);
    let mut access = AccessArgs::default();
    let mut route: Option<LitStr> = None;
    let mut sensitive = false;
    let mut reason = false;
    let mut ai = AiArgs::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("permission") {
            access.permission = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("public") {
            access.public = true;
        } else if meta.path.is_ident("route") {
            route = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("sensitive") {
            sensitive = true;
        } else if meta.path.is_ident("reason") {
            reason = true;
        } else if meta.path.is_ident("ai") {
            ai.parse_ai(&meta)?;
        } else if meta.path.is_ident("confirm") {
            return Err(meta.error("`confirm` belongs on the action that writes\n  = help: put `confirm` on the `#[action]` that the screen's form or button runs"));
        } else {
            return Err(meta.error(
                "unknown screen option\n  = help: expected `route = \"/...\"`, `permission = \"...\"`, `public`, `sensitive`, `reason`, `ai = \"...\"` or `ai = false`",
            ));
        }
        Ok(())
    });
    syn::parse_macro_input!(attr with parser);
    let original = quote!(#f);
    or_error(expand_screen(&f, &access, route.as_ref(), (sensitive || reason, reason), &ai), original)
}

fn expand_screen(
    f: &ItemFn,
    access: &AccessArgs,
    route: Option<&LitStr>,
    privacy: (bool, bool),
    ai: &AiArgs,
) -> syn::Result<Ts> {
    let ai = ai.tokens("screen")?;
    let doc = doc_text(&f.attrs);
    let (sensitive, reason) = privacy;
    if reason && access.public {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            "a public screen cannot ask for a reason\n  = help: give the screen a permission",
        ));
    }
    let (params, _) = fn_types(f, "screen")?;
    let name = &f.sig.ident;
    let access = access.tokens("screen", name)?;
    let path_value = match route {
        Some(r) => path_value_fn(r, &params, params.to_string().replace(char::is_whitespace, "") == "()")?,
        None => quote!(),
    };
    let route = match route {
        Some(r) if !r.value().starts_with('/') => {
            return Err(syn::Error::new(r.span(), "a screen route must start with `/`"));
        }
        Some(r) => quote!(::core::option::Option::Some(#r)),
        None => quote!(::core::option::Option::None),
    };
    let name_str = name.to_string();
    let base = unit_and_run(f);
    let (binds, call, deps) = injected_call(f, "screen")?;
    Ok(quote! {
        #base

        impl ::zeyn::ScreenDef for #name {
            type Params = #params;
            const META: ::zeyn::ScreenMeta = ::zeyn::ScreenMeta {
                name: #name_str,
                route: #route,
                access: #access,
                sensitive: #sensitive,
                reason: #reason,
                doc: #doc,
                ai: #ai,
            };

            fn deps() -> ::std::vec::Vec<::zeyn::Dep> {
                #deps
            }

            #path_value

            fn build(ctx: ::zeyn::Ctx, input: Self::Params) -> ::zeyn::BoxFuture<::zeyn::Result<::zeyn::Screen>> {
                ::std::boxed::Box::pin(async move {
                    #binds
                    ::zeyn::IntoScreen::into_screen(#call)
                })
            }
        }
    })
}

/// `ScreenDef::path_value` for a route with path segments (`/orders/{id}`): each segment names a
/// params field and reads as that field's type. Nothing for a route without segments.
fn path_value_fn(route: &LitStr, params: &Ts, no_params: bool) -> syn::Result<Ts> {
    let value = route.value();
    let mut names: Vec<String> = Vec::new();
    for seg in value.split('/').filter(|s| s.contains('{') || s.contains('}')) {
        let name = seg.strip_prefix('{').and_then(|s| s.strip_suffix('}')).filter(|n| syn::parse_str::<Ident>(n).is_ok());
        let Some(name) = name else {
            return Err(syn::Error::new(
                route.span(),
                format!(
                    "route segment `{seg}` must be a whole segment `{{field}}` that names a field of the screen's params
  = help: write `/orders/{{id}}` with a field `id` in the `#[params]` struct; wildcards and partial segments such as `{{a}}-{{b}}` are not supported"
                ),
            ));
        };
        if names.iter().any(|n| n == name) {
            return Err(syn::Error::new(
                route.span(),
                format!("route names `{{{name}}}` twice
  = help: each path segment binds a different params field"),
            ));
        }
        names.push(name.to_string());
    }
    if names.is_empty() {
        return Ok(quote!());
    }
    if no_params {
        return Err(syn::Error::new(
            route.span(),
            format!(
                "route `{value}` has path segments but the screen takes no params
  = help: add a `#[params]` struct with a field `{}` and take it as the screen's first argument",
                names[0]
            ),
        ));
    }
    let arms = names.iter().map(|n| {
        let field = Ident::new(n, route.span());
        quote_spanned! {route.span()=>
            #n => ::zeyn::__path_value(raw, |p: &#params| &p.#field),
        }
    });
    Ok(quote! {
        fn path_value(field: &str, raw: &str) -> ::core::option::Option<::zeyn::__serde_json::Value> {
            match field {
                #(#arms)*
                _ => ::core::option::Option::None,
            }
        }
    })
}

/// Marks a struct or enum as screen params.
#[proc_macro_attribute]
pub fn params(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(Ts::from(attr).span(), "`#[params]` takes no options").to_compile_error().into();
    }
    let item = syn::parse_macro_input!(item as Item);
    let ident = match &item {
        Item::Struct(s) => &s.ident,
        Item::Enum(e) => &e.ident,
        other => {
            return syn::Error::new(other.span(), "`#[params]` can only be used on a struct or an enum")
                .to_compile_error()
                .into();
        }
    };
    quote! {
        #[derive(::zeyn::__serde::Serialize, ::zeyn::__serde::Deserialize, Clone, Debug)]
        #[serde(crate = "::zeyn::__serde")]
        #item

        impl ::zeyn::Params for #ident {}

        impl ::zeyn::ApiInput for #ident {
            fn schema() -> ::zeyn::__serde_json::Value {
                ::zeyn::__serde_json::json!({ "type": "object", "title": stringify!(#ident) })
            }

            fn from_json(value: ::zeyn::__serde_json::Value) -> ::zeyn::Result<Self> {
                ::zeyn::__serde_json::from_value(value).map_err(|_| ::zeyn::Error::NotFound)
            }
        }
    }
    .into()
}

/// Defines a field type with a fixed set of options (a select list). Variants may take `#[label("...")]`.
#[proc_macro_attribute]
pub fn choice(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(Ts::from(attr).span(), "`#[choice]` takes no options").to_compile_error().into();
    }
    let e = syn::parse_macro_input!(item as ItemEnum);
    let original = quote!(#e);
    or_error(expand_choice(e), original)
}

fn expand_choice(mut e: ItemEnum) -> syn::Result<Ts> {
    if e.variants.is_empty() {
        return Err(syn::Error::new(e.ident.span(), "a choice needs at least one variant"));
    }
    let enum_name = e.ident.to_string();
    let has_default = e.variants.iter().any(|v| v.attrs.iter().any(|a| a.path().is_ident("default")));
    let mut options = Vec::new();
    let mut arms = Vec::new();
    for (i, v) in e.variants.iter_mut().enumerate() {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(v.span(), "choice variants cannot have fields"));
        }
        let mut label: Option<LitStr> = None;
        let mut keep = Vec::new();
        for a in v.attrs.drain(..) {
            if a.path().is_ident("label") {
                label = Some(a.parse_args()?);
            } else {
                keep.push(a);
            }
        }
        if i == 0 && !has_default {
            keep.push(syn::parse_quote!(#[default]));
        }
        v.attrs = keep;
        let ident = &v.ident;
        let value = ident.to_string();
        let label = label.map(|l| l.value()).unwrap_or_else(|| value.clone());
        let key = format!("{enum_name}.{value}");
        options.push(quote!(::zeyn::ChoiceOption { value: #value, label: #label, key: #key }));
        arms.push(quote!(Self::#ident => #value));
    }
    let ident = &e.ident;
    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, ::zeyn::__serde::Serialize, ::zeyn::__serde::Deserialize)]
        #[serde(crate = "::zeyn::__serde")]
        #e

        impl ::zeyn::ChoiceType for #ident {}

        impl #ident {
            #[doc(hidden)]
            pub const __ZEYN_OPTIONS: &'static [::zeyn::ChoiceOption] = &[#(#options),*];
        }

        impl ::zeyn::FieldType for #ident {
            const KIND: ::zeyn::FieldKind = ::zeyn::FieldKind::Choice(Self::__ZEYN_OPTIONS);
            fn to_value(&self) -> ::zeyn::Value {
                ::zeyn::Value::Choice(match self { #(#arms),* })
            }
        }

        impl ::zeyn::VecItem for #ident {
            const VEC_KIND: ::zeyn::FieldKind = ::zeyn::FieldKind::Choices(Self::__ZEYN_OPTIONS);
            fn __vec_value(items: &[Self]) -> ::zeyn::Value {
                ::zeyn::__choices_value(items)
            }
        }
    })
}

/// Options shared by `#[model]` and `#[entity]`.
#[derive(Default)]
struct ModelArgs {
    table: Option<LitStr>,
    validate: Option<Path>,
    sensitive: bool,
}

fn parse_model_args(attr: TokenStream, allow_table: bool) -> syn::Result<ModelArgs> {
    let mut args = ModelArgs::default();
    let parser = syn::meta::parser(|meta| {
        if allow_table && meta.path.is_ident("table") {
            args.table = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("validate") {
            args.validate = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("sensitive") {
            args.sensitive = true;
        } else if allow_table {
            return Err(meta.error(
                "unknown entity option\n  = help: expected `table = \"...\"`, `validate = path::to::fn` or `sensitive`",
            ));
        } else {
            return Err(meta.error("unknown model option\n  = help: expected `validate = path::to::fn` or `sensitive`"));
        }
        Ok(())
    });
    syn::parse::Parser::parse(parser, attr)?;
    Ok(args)
}

/// Defines validated data that is not tied to a table: form input, filters, wizard steps, report rows.
///
/// Field rules: `required`, `min_len = N`, `max_len = N`, `email`, `min = N`, `max = N`,
/// `scale = N` and `precision = N` (for `Decimal`), `mask = "..."` (for text).
/// Rules that involve several fields go in a function given with `validate = ...`.
///
/// Text fields with personal data can be masked: `#[mask(last4)]`, `#[mask(email)]` or
/// `#[mask]` (nothing shown). Users with `<model>.<field>.unmask` (or the permission given with
/// `#[mask(last4, permission = "...")]`) may reveal the value; every reveal is audited.
///
/// ```ignore
/// #[model(validate = check_dates)]
/// pub struct LeaveRequest {
///     #[rule(required)]
///     pub reason: String,
///     pub starts: i64,
///     pub ends: i64,
/// }
/// ```
#[proc_macro_attribute]
pub fn model(attr: TokenStream, item: TokenStream) -> TokenStream {
    let s = syn::parse_macro_input!(item as ItemStruct);
    let original = quote!(#s);
    let result = parse_model_args(attr, false).and_then(|args| expand_model(s, args, false));
    or_error(result, original)
}

/// Defines a model stored in a database table. Needs `table = "..."` and exactly one `#[key]` field.
///
/// Generates typed field references (`Product::code`), a `ProductId` key type, validation and a schema.
///
/// ```ignore
/// #[entity(table = "products")]
/// pub struct Product {
///     #[key]
///     pub id: ProductId,
///     #[rule(required, max_len = 20)]
///     pub code: String,
/// }
/// ```
#[proc_macro_attribute]
pub fn entity(attr: TokenStream, item: TokenStream) -> TokenStream {
    let s = syn::parse_macro_input!(item as ItemStruct);
    let original = quote!(#s);
    let result = parse_model_args(attr, true).and_then(|args| expand_model(s, args, true));
    or_error(result, original)
}

/// `LeaveRequest` -> `leave_request`.
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `unit_price` -> `Unit price`.
fn default_label(name: &str) -> String {
    let text = name.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => text,
    }
}

fn expand_model(mut s: ItemStruct, args: ModelArgs, is_entity: bool) -> syn::Result<Ts> {
    let name = s.ident.clone();
    let kind = if is_entity { "entity" } else { "model" };
    let before = s.attrs.len();
    s.attrs.retain(|a| !a.path().is_ident("sensitive"));
    let sensitive = args.sensitive || s.attrs.len() != before;
    if !s.generics.params.is_empty() {
        return Err(syn::Error::new(s.generics.span(), format!("{kind}s cannot be generic")));
    }
    let Fields::Named(named) = &mut s.fields else {
        return Err(syn::Error::new(name.span(), format!("a {kind} must have named fields")));
    };

    let id_name = format_ident!("{}Id", name);
    let mut metas = Vec::new();
    let mut getters = Vec::new();
    let mut consts = Vec::new();
    let mut validations = Vec::new();
    let mut value_arms = Vec::new();
    let mut checks = Vec::new();
    let mut keys = 0usize;
    let mut generate_id = false;

    for (idx, field) in named.named.iter_mut().enumerate() {
        let fname = field.ident.clone().expect("named field");
        let fname_str = fname.to_string();
        let ty: Type = field.ty.clone();
        let fvis = field.vis.clone();
        let mut is_key = false;
        let mut readonly = false;
        let mut version = false;
        let mut label: Option<LitStr> = None;
        let mut mask = quote!(::core::option::Option::None);
        let mut masked = false;
        let mut rules = Vec::new();
        let mut keep = Vec::new();

        for a in field.attrs.drain(..) {
            if a.path().is_ident("key") {
                is_key = true;
            } else if a.path().is_ident("readonly") {
                readonly = true;
            } else if a.path().is_ident("version") {
                version = true;
                checks.push(quote_spanned!(ty.span()=> ::zeyn::__rules::version::<#ty>();));
            } else if a.path().is_ident("label") {
                label = Some(a.parse_args()?);
            } else if a.path().is_ident("mask") {
                let mut style = quote!(::zeyn::MaskStyle::Full);
                let mut permission: Option<LitStr> = None;
                if !matches!(a.meta, syn::Meta::Path(_)) {
                    a.parse_nested_meta(|m| {
                        if m.path.is_ident("last4") {
                            style = quote!(::zeyn::MaskStyle::LastFour);
                        } else if m.path.is_ident("email") {
                            style = quote!(::zeyn::MaskStyle::Email);
                        } else if m.path.is_ident("full") {
                            style = quote!(::zeyn::MaskStyle::Full);
                        } else if m.path.is_ident("permission") {
                            let p: LitStr = m.value()?.parse()?;
                            check_permission(&p)?;
                            permission = Some(p);
                        } else {
                            return Err(m.error("unknown mask option\n  = help: expected `last4`, `email`, `full` or `permission = \"...\"`"));
                        }
                        Ok(())
                    })?;
                }
                let permission = permission
                    .map(|p| p.value())
                    .unwrap_or_else(|| format!("{}.{}.unmask", snake_case(&name.to_string()), fname_str));
                checks.push(quote_spanned!(ty.span()=> ::zeyn::__rules::text::<#ty>();));
                mask = quote!(::core::option::Option::Some(::zeyn::Masking { style: #style, permission: #permission }));
                masked = true;
            } else if a.path().is_ident("rule") {
                a.parse_nested_meta(|m| {
                    let text_check = quote_spanned!(ty.span()=> ::zeyn::__rules::text::<#ty>(););
                    if m.path.is_ident("required") {
                        rules.push(quote!(::zeyn::Rule::Required));
                        checks.push(quote_spanned!(ty.span()=> ::zeyn::__rules::required::<#ty>();));
                    } else if m.path.is_ident("unique") {
                        rules.push(quote!(::zeyn::Rule::Unique));
                    } else if m.path.is_ident("email") {
                        rules.push(quote!(::zeyn::Rule::Email));
                        checks.push(text_check);
                    } else if m.path.is_ident("embed") {
                        rules.push(quote!(::zeyn::Rule::Embed));
                        checks.push(text_check);
                    } else if m.path.is_ident("max_len") || m.path.is_ident("min_len") {
                        let n: LitInt = m.value()?.parse()?;
                        if m.path.is_ident("max_len") {
                            rules.push(quote!(::zeyn::Rule::MaxLen(#n)));
                        } else {
                            rules.push(quote!(::zeyn::Rule::MinLen(#n)));
                        }
                        checks.push(text_check);
                    } else if m.path.is_ident("min") || m.path.is_ident("max") {
                        let n: syn::Expr = m.value()?.parse()?;
                        if m.path.is_ident("min") {
                            rules.push(quote!(::zeyn::Rule::Min(#n)));
                        } else {
                            rules.push(quote!(::zeyn::Rule::Max(#n)));
                        }
                        checks.push(quote_spanned!(ty.span()=> ::zeyn::__rules::min_max::<#ty>();));
                    } else if m.path.is_ident("scale") || m.path.is_ident("precision") {
                        let n: LitInt = m.value()?.parse()?;
                        let v: u8 = n.base10_parse()?;
                        if m.path.is_ident("scale") {
                            if v > 28 {
                                return Err(syn::Error::new(n.span(), "`scale` is at most 28"));
                            }
                            rules.push(quote!(::zeyn::Rule::Scale(#n)));
                        } else {
                            if v == 0 || v > 38 {
                                return Err(syn::Error::new(n.span(), "`precision` is between 1 and 38"));
                            }
                            rules.push(quote!(::zeyn::Rule::Precision(#n)));
                        }
                        checks.push(quote_spanned!(ty.span()=> ::zeyn::__rules::scale::<#ty>();));
                    } else if m.path.is_ident("mask") {
                        let p: LitStr = m.value()?.parse()?;
                        let mut slots = 0usize;
                        let mut escaped = false;
                        for c in p.value().chars() {
                            if !escaped && matches!(c, '9' | 'a' | 'A' | '*') {
                                slots += 1;
                            }
                            escaped = !escaped && c == '\\';
                        }
                        if slots == 0 {
                            return Err(syn::Error::new(
                                p.span(),
                                "a mask needs at least one slot\n  = help: `9` is a digit, `a` a letter, `A` an upper case letter, `*` a letter or digit",
                            ));
                        }
                        rules.push(quote!(::zeyn::Rule::Mask(#p)));
                        checks.push(text_check);
                    } else {
                        return Err(m.error(
                            "unknown rule\n  = help: expected `required`, `unique`, `email`, `embed`, `min_len = N`, `max_len = N`, `min = N`, `max = N`, `scale = N`, `precision = N` or `mask = \"...\"`",
                        ));
                    }
                    Ok(())
                })?;
            } else {
                keep.push(a);
            }
        }
        let guard = if masked {
            Some((format_ident!("__zeyn_mask_{}", fname), quote!(::zeyn::__privacy::masked_output)))
        } else if is_key && sensitive {
            Some((format_ident!("__zeyn_read_{}", fname), quote!(::zeyn::__privacy::read_output)))
        } else {
            None
        };
        if let Some((with, call)) = guard {
            let path = LitStr::new(&format!("{name}::{with}"), fname.span());
            keep.push(syn::parse_quote!(#[serde(serialize_with = #path)]));
            getters.push(quote! {
                #[doc(hidden)]
                fn #with<S: ::zeyn::__serde::Serializer>(value: &#ty, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                    #call(value, &Self::__ZEYN_FIELDS[#idx], s)
                }
            });
        }
        field.attrs = keep;

        if is_key {
            keys += 1;
            if let Type::Path(p) = &ty {
                if p.qself.is_none() && p.path.is_ident(&id_name) {
                    generate_id = true;
                }
            }
        }
        if is_key && !is_entity {
            return Err(syn::Error::new(
                fname.span(),
                "`#[key]` is only allowed on entities\n  = help: use `#[entity(table = \"...\")]` for table backed data",
            ));
        }

        let label = label.map(|l| l.value()).unwrap_or_else(|| default_label(&fname_str));
        let getter = format_ident!("__zeyn_get_{}", fname);
        let model_str = name.to_string();
        metas.push(quote! {
            ::zeyn::FieldMeta {
                model: #model_str,
                name: #fname_str,
                label: #label,
                kind: <#ty as ::zeyn::FieldType>::KIND,
                rules: &[#(#rules),*],
                key: #is_key,
                readonly: #readonly,
                version: #version,
                optional: <#ty as ::zeyn::FieldType>::OPTIONAL,
                mask: #mask,
            }
        });
        getters.push(quote! {
            #[doc(hidden)]
            fn #getter(m: &#name) -> &#ty { &m.#fname }
        });
        consts.push(quote! {
            #fvis const #fname: ::zeyn::Field<#name, #ty> = ::zeyn::Field::new(&Self::__ZEYN_FIELDS[#idx], Self::#getter);
        });
        validations.push(quote! {
            ::zeyn::__rules::validate(&self.#fname, &Self::__ZEYN_FIELDS[#idx], &mut errors);
        });
        value_arms.push(quote!(#idx => ::core::option::Option::Some(::zeyn::FieldType::to_value(&self.#fname))));
    }

    let table_impl = if is_entity {
        let table = args.table.ok_or_else(|| {
            syn::Error::new(name.span(), format!("entity `{name}` has no table\n  = help: add `table = \"...\"`"))
        })?;
        if keys != 1 {
            return Err(syn::Error::new(
                name.span(),
                format!("entity `{name}` needs exactly one `#[key]` field, found {keys}"),
            ));
        }
        Some(quote! {
            impl ::zeyn::Entity for #name {
                const TABLE: &'static str = #table;
            }
        })
    } else {
        None
    };

    let custom = args.validate.map(|path| quote!(#path(self, &mut errors);));
    let vis = &s.vis;
    let id_struct = generate_id.then(|| {
        let doc = format!("Key of a `{name}` record.");
        quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, ::zeyn::__serde::Serialize, ::zeyn::__serde::Deserialize)]
            #[serde(crate = "::zeyn::__serde", transparent)]
            #vis struct #id_name(pub i64);

            impl ::zeyn::FieldType for #id_name {
                const KIND: ::zeyn::FieldKind = ::zeyn::FieldKind::Id;
                fn to_value(&self) -> ::zeyn::Value { ::zeyn::Value::Int(self.0) }
            }

            impl ::zeyn::ui::LookupValue for #id_name {}

            impl ::zeyn::PathValue for #id_name {
                fn from_segment(raw: &str) -> ::core::option::Option<Self> {
                    <i64 as ::zeyn::PathValue>::from_segment(raw).map(#id_name)
                }
            }

            impl ::zeyn::ApiInput for #id_name {
                fn schema() -> ::zeyn::__serde_json::Value {
                    ::zeyn::__serde_json::json!({ "type": "integer", "title": stringify!(#id_name) })
                }

                fn from_json(value: ::zeyn::__serde_json::Value) -> ::zeyn::Result<Self> {
                    ::zeyn::__serde_json::from_value(value).map_err(|_| ::zeyn::Error::NotFound)
                }
            }
        }
    });
    let name_str = name.to_string();

    Ok(quote! {
        #[derive(Clone, Debug, Default, ::zeyn::__serde::Serialize, ::zeyn::__serde::Deserialize)]
        #[serde(crate = "::zeyn::__serde")]
        #s

        #id_struct

        #[allow(non_upper_case_globals)]
        impl #name {
            #[doc(hidden)]
            pub const __ZEYN_FIELDS: &'static [::zeyn::FieldMeta] = &[#(#metas),*];
            #(#getters)*
            #(#consts)*
        }

        impl ::zeyn::Model for #name {
            const NAME: &'static str = #name_str;
            const SENSITIVE: bool = #sensitive;
            fn fields() -> &'static [::zeyn::FieldMeta] { Self::__ZEYN_FIELDS }
            fn value_at(&self, index: usize) -> ::core::option::Option<::zeyn::Value> {
                match index {
                    #(#value_arms,)*
                    _ => ::core::option::Option::None,
                }
            }
            fn validate(&self) -> ::core::result::Result<(), ::zeyn::ValidationErrors> {
                let mut errors = ::zeyn::ValidationErrors::default();
                #(#validations)*
                #custom
                errors.into_result()
            }
        }

        #table_impl

        const _: () = {
            #[allow(dead_code)]
            fn __zeyn_rule_checks() { #(#checks)* }
        };
    })
}

/// Catalog text checked at compile time: `t!("products.title")` or `t!("orders.count", n = count)`.
///
/// Every JSON file in the crate's `locales/` folder must have the key, and the message's `{name}`
/// placeholders must match the arguments exactly.
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(input as TArgs);
    expand_t(args).unwrap_or_else(|e| e.to_compile_error()).into()
}

struct TArgs {
    key: LitStr,
    args: Vec<(Ident, syn::Expr)>,
}

impl syn::parse::Parse for TArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let key: LitStr = input.parse()?;
        let mut args = Vec::new();
        while input.parse::<syn::Token![,]>().is_ok() {
            if input.is_empty() {
                break;
            }
            let name: Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            let value: syn::Expr = input.parse()?;
            args.push((name, value));
        }
        Ok(TArgs { key, args })
    }
}

fn lookup_key<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(value, |v, part| v.get(part))?.as_str()
}

fn placeholders(message: &str) -> std::collections::BTreeSet<String> {
    let mut found = std::collections::BTreeSet::new();
    let mut rest = message;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let name = &rest[start + 1..start + len];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            found.insert(name.to_string());
        }
        rest = &rest[start + len..];
    }
    found
}

fn expand_t(t: TArgs) -> syn::Result<Ts> {
    let key = t.key.value();
    let span = t.key.span();
    let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("locales");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|_| {
            syn::Error::new(
                span,
                format!("t!: no `locales` folder at {}\n  = help: create locales/en.json", dir.display()),
            )
        })?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(syn::Error::new(span, "t!: the `locales` folder has no .json files"));
    }
    let given: std::collections::BTreeSet<String> = t.args.iter().map(|(n, _)| n.to_string()).collect();
    let mut tracked = Vec::new();
    for path in &files {
        let text =
            std::fs::read_to_string(path).map_err(|e| syn::Error::new(span, format!("t!: {}: {e}", path.display())))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| syn::Error::new(span, format!("t!: {} is not valid JSON: {e}", path.display())))?;
        let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let Some(message) = lookup_key(&json, &key) else {
            return Err(syn::Error::new(
                span,
                format!("t!: key `{key}` is missing in locales/{file}\n  = help: add it to every file in locales/"),
            ));
        };
        let wanted = placeholders(message);
        if let Some(missing) = wanted.difference(&given).next() {
            return Err(syn::Error::new(
                span,
                format!("t!: `{key}` in locales/{file} uses {{{missing}}} but no `{missing} = ...` argument was given"),
            ));
        }
        if let Some(extra) = given.difference(&wanted).next() {
            return Err(syn::Error::new(
                span,
                format!("t!: argument `{extra}` is not used by `{key}` in locales/{file}"),
            ));
        }
        let p = path.to_string_lossy().to_string();
        tracked.push(quote!(
            const _: &[u8] = ::core::include_bytes!(#p);
        ));
    }
    let args = t.args.iter().map(|(n, v)| {
        let n = n.to_string();
        quote!(.arg(#n, ::zeyn::Text::from(::std::string::ToString::to_string(&#v))))
    });
    Ok(quote!({
        #(#tracked)*
        ::zeyn::Text::key(#key) #(#args)*
    }))
}
