//! `counter` island: a value with less and more buttons, saved on the server through a typed
//! action call. The smallest example of the island pattern: props in, a remote out.

use showcase_shared::{Count, CounterProps, SaveCount};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element};
use zeyn_island::Island;

/// Called by `/_zeyn/islands.js` with the placeholder element.
#[wasm_bindgen]
pub fn mount(root: Element) -> Result<(), JsValue> {
    let island = Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    let doc = root.owner_document().ok_or("the island has no document")?;
    let texts = Rc::new(props.texts);
    let value = Rc::new(Cell::new(props.start));

    let bar = make(&doc, "div", "z-row z-row-compact")?;
    bar.set_attribute("role", "group")?;
    bar.set_attribute("aria-label", &texts.value)?;
    let less = make(&doc, "button", "z-button")?;
    less.set_text_content(Some(&texts.less));
    let shown = make(&doc, "output", "")?;
    shown.set_attribute("aria-live", "polite")?;
    shown.set_attribute("data-testid", "counter.value")?;
    shown.set_text_content(Some(&props.start.to_string()));
    let more = make(&doc, "button", "z-button")?;
    more.set_text_content(Some(&texts.more));
    let save = make(&doc, "button", "z-button z-primary")?;
    save.set_text_content(Some(&texts.save));
    let status = make(&doc, "span", "")?;
    status.set_attribute("role", "status")?;
    status.set_attribute("data-testid", "counter.status")?;
    for part in [&less, &shown, &more, &save, &status] {
        bar.append_child(part)?;
    }
    root.append_child(&bar)?;

    for (button, step) in [(&less, -1), (&more, 1)] {
        let (value, shown) = (value.clone(), shown.clone());
        on_click(button, move || {
            value.set(value.get().saturating_add(step));
            shown.set_text_content(Some(&value.get().to_string()));
        })?;
    }
    on_click(&save, move || {
        let (island, status, texts) = (island.clone(), status.clone(), texts.clone());
        let count = Count { value: value.get() };
        zeyn_island::spawn(async move {
            let text = match island.call::<SaveCount>(&count).await {
                Ok(saved) => texts.saved.replace("{value}", &saved.value.to_string()),
                Err(e) => e.message,
            };
            status.set_text_content(Some(&text));
        });
    })
}

fn make(doc: &Document, tag: &str, class: &str) -> Result<Element, JsValue> {
    let el = doc.create_element(tag)?;
    if tag == "button" {
        el.set_attribute("type", "button")?;
    }
    if !class.is_empty() {
        el.set_class_name(class);
    }
    Ok(el)
}

/// Runs `f` on every click of `target`. The listener lives as long as the page.
fn on_click(target: &Element, f: impl FnMut() + 'static) -> Result<(), JsValue> {
    let cb = Closure::<dyn FnMut()>::new(f);
    target.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
    cb.forget();
    Ok(())
}
