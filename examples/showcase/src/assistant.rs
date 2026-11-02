//! The embedded assistant, with a scripted model by default so the tour runs offline and costs
//! nothing. Point it at a real model with configuration only (environment variables shown):
//!
//! - OpenAI or any compatible server (Ollama, vLLM, LiteLLM, Azure OpenAI v1):
//!   `NORTHWIND_ASSISTANT__PROVIDER=openai`, `NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1`,
//!   `NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini`, `NORTHWIND_ASSISTANT__API_KEY=...`
//!   (add `NORTHWIND_ASSISTANT__AZURE=true` for Azure's `api-key` header)
//! - Anthropic: `NORTHWIND_ASSISTANT__PROVIDER=anthropic`, `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5`,
//!   `NORTHWIND_ASSISTANT__API_KEY=...`
//! - Off: `NORTHWIND_ASSISTANT__PROVIDER=off`

use crate::{sales, stock};
use serde_json::json;
use std::time::Duration;
use zeyn::Config;
use zeyn::assistant::{Anthropic, Assistant, AssistantChat, OpenAiCompatible, Say, ScriptedProvider};
use zeyn::prelude::*;

/// Talk to the app: ask about orders and products, open lists, archive a product.
#[screen(route = "/assistant", permission = "stock.product.read", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new(t!("assistant.title")).body(Alert::info(t!("assistant.intro"))).body(AssistantChat::page())
}

/// A scripted model that walks through the tour: filter today's orders, archive a product.
fn scripted() -> ScriptedProvider {
    ScriptedProvider::new()
        .delay(Duration::from_millis(25))
        .when(
            "izmir",
            vec![
                vec![Say::tool("navigate", json!({ "screen": "today", "filters": [{ "field": "city", "contains": "İzmir" }] }))],
                vec![
                    Say::text("Here are **today's orders from İzmir**. I filtered the list by city; you can change the filter in the column header."),
                    Say::tool("suggest", json!({ "items": ["Archive product NW-0003", "Show products low on stock"] })),
                ],
            ],
        )
        .when(
            "archive",
            vec![
                vec![Say::text("I will archive **NW-0003**. Please confirm below."), Say::tool("api_stock_archive_product", json!("NW-0003"))],
                vec![Say::text("Done: product `NW-0003` is archived. It stays in reports but can no longer be sold.")],
            ],
        )
        .when(
            "low stock",
            vec![
                vec![Say::tool("navigate", json!({ "screen": "product_list", "filters": [{ "field": "stock", "to": "9" }] }))],
                vec![Say::text("These products have fewer than 10 units in stock.")],
            ],
        )
        .otherwise(vec![vec![
            Say::text("This demo uses a scripted model. Try one of these:\n- Show me today's orders in İzmir\n- Archive product NW-0003\n- Show products low on stock"),
        ]])
}

/// The assistant from configuration: `assistant.provider` is `scripted` (default), `openai`,
/// `anthropic` or `off`.
pub fn assistant(config: &Config) -> std::io::Result<Option<Assistant>> {
    let model = config.get("assistant.model").unwrap_or("").to_string();
    let key = || config.secret("assistant.api_key").map_err(std::io::Error::other);
    let assistant = match config.get("assistant.provider").unwrap_or("scripted") {
        "off" => return Ok(None),
        "openai" => {
            let base = config.get("assistant.base_url").unwrap_or("https://api.openai.com/v1");
            let mut p = OpenAiCompatible::new(base, model);
            if config.get("assistant.api_key").is_some() {
                p = p.api_key(key()?);
            }
            if config.get("assistant.azure") == Some("true") {
                p = p.azure();
            }
            Assistant::new(p)
        }
        "anthropic" => Assistant::new(Anthropic::new(model).api_key(key()?)),
        _ => Assistant::new(scripted()),
    };
    Ok(Some(
        assistant
            .instructions("Northwind sells food and drink to shops across Türkiye. Users manage stock, orders and staff leave.")
            .suggest(assistant_page, t!("assistant.try_orders"))
            .suggest(assistant_page, t!("assistant.try_archive"))
            .suggest(sales::today, t!("assistant.try_orders"))
            .suggest(stock::product_list, t!("assistant.try_low")),
    ))
}

pub fn feature() -> Feature {
    Feature::new("assistant")
        .guard(SignedIn)
        .screen(assistant_page)
        .menu(|m| m.group(t!("menu.assistant"), Icon::Search).item(t!("assistant.title"), assistant_page))
}
