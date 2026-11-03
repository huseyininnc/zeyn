//! `diagram` island: a small diagram editor on the Zeyn canvas surface.
//!
//! Pointer: drag a shape to move it, drag the background to pan, wheel to zoom around the
//! pointer. Keyboard (canvas focused): `]` and `[` select the next or previous shape, arrows move
//! it (Shift for 10 px), Delete removes it, `+`, `-` and `0` zoom, Escape clears the selection.
//! The toolbar buttons do the same with ordinary buttons, and the list under the canvas is the
//! text version of the picture. Save calls the server through a typed action. With `live` in the
//! props the editor joins a Y-CRDT document over the realtime channel, so several people edit
//! the same diagram at once.

use showcase_shared::{Diagram, DiagramProps, DiagramTexts, SaveDiagram, Shape, ShapeKind, crdt};
use std::cell::RefCell;
use std::f64::consts::TAU;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Document, Element};
use zeyn_island::Island;
use zeyn_island::canvas::{Frame, Graphics, Input, Prefer, Scene, Surface};
use zeyn_island::live::Channel;

/// What the pointer is holding.
enum Grab {
    Shape { dx: f64, dy: f64 },
    Pan { x: f64, y: f64 },
}

/// The DOM around the canvas that mirrors the scene for every user.
struct View {
    summary: Element,
    status: Element,
    texts: Rc<DiagramTexts>,
}

impl View {
    fn refresh(&self, diagram: &Diagram, selected: Option<usize>) {
        let doc = self.summary.owner_document();
        self.summary.set_text_content(None);
        let lines = diagram.summary(&self.texts);
        if lines.is_empty() {
            self.summary
                .set_text_content(Some(&self.texts.empty));
        }
        for (i, line) in lines.iter().enumerate() {
            if let Some(li) = doc
                .as_ref()
                .and_then(|d| d.create_element("li").ok())
            {
                li.set_text_content(Some(line));
                if selected == Some(i) {
                    let _ = li.set_attribute("aria-current", "true");
                }
                let _ = self.summary.append_child(&li);
            }
        }
    }

    fn say(&self, text: &str) {
        self.status.set_text_content(Some(text));
    }
}

/// The editor scene.
pub struct Editor {
    diagram: Diagram,
    selected: Option<usize>,
    grab: Option<Grab>,
    zoom: f64,
    pan: (f64, f64),
    host: Element,
    view: View,
    /// Runs after every change made here (not for changes from others), to share it.
    on_edit: Option<Rc<dyn Fn(&Diagram)>>,
}

impl Editor {
    fn world(&self, x: f64, y: f64) -> (f64, f64) {
        ((x - self.pan.0) / self.zoom, (y - self.pan.1) / self.zoom)
    }

    fn hit(&self, x: f64, y: f64) -> Option<usize> {
        let (wx, wy) = self.world(x, y);
        self.diagram
            .shapes
            .iter()
            .rposition(|s| {
                wx >= f64::from(s.x)
                    && wx <= f64::from(s.x + s.w)
                    && wy >= f64::from(s.y)
                    && wy <= f64::from(s.y + s.h)
            })
    }

    fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        self.changed();
        if let Some(line) = index.and_then(|i| {
            self.diagram
                .summary(&self.view.texts)
                .get(i)
                .cloned()
        }) {
            self.view.say(
                &self
                    .view
                    .texts
                    .selected
                    .replace("{shape}", &line),
            );
        }
    }

    fn changed(&self) {
        self.view
            .refresh(&self.diagram, self.selected);
        if let Some(share) = &self.on_edit {
            share(&self.diagram);
        }
    }

    /// Takes the diagram as others left it, keeping the selected shape selected.
    fn replace(&mut self, diagram: Diagram) {
        let id = self
            .selected
            .and_then(|i| self.diagram.shapes.get(i))
            .map(|s| s.id);
        self.diagram = diagram;
        self.selected = id.and_then(|id| {
            self.diagram
                .shapes
                .iter()
                .position(|s| s.id == id)
        });
        self.view
            .refresh(&self.diagram, self.selected);
    }

    fn add(&mut self, kind: ShapeKind) {
        if self.diagram.shapes.len() >= showcase_shared::MAX_SHAPES {
            return;
        }
        let id = loop {
            let id = (js_sys::Math::random() * f64::from(u32::MAX)) as u32;
            if id > 0
                && !self
                    .diagram
                    .shapes
                    .iter()
                    .any(|s| s.id == id)
            {
                break id;
            }
        };
        let n = self.diagram.shapes.len() as i32;
        let label = match kind {
            ShapeKind::Box => format!("{} {}", self.view.texts.kind_box, n + 1),
            ShapeKind::Ellipse => format!("{} {}", self.view.texts.kind_ellipse, n + 1),
        };
        self.diagram.shapes.push(Shape {
            id,
            kind,
            x: 40 + (n % 6) * 30,
            y: 40 + (n % 6) * 30,
            w: 140,
            h: 64,
            label,
        });
        self.select(Some(self.diagram.shapes.len() - 1));
    }

    fn remove(&mut self) {
        if let Some(i) = self
            .selected
            .filter(|i| *i < self.diagram.shapes.len())
        {
            self.diagram.shapes.remove(i);
            self.selected = None;
            self.changed();
        }
    }

    fn step(&mut self, by: isize) {
        let n = self.diagram.shapes.len() as isize;
        if n == 0 {
            return;
        }
        let next = match self.selected {
            Some(i) => (i as isize + by).rem_euclid(n),
            None if by > 0 => 0,
            None => n - 1,
        };
        self.select(Some(next as usize));
    }

    fn nudge(&mut self, dx: i32, dy: i32) -> bool {
        let Some(s) = self
            .selected
            .and_then(|i| self.diagram.shapes.get_mut(i))
        else {
            return false;
        };
        s.x = (s.x + dx).clamp(0, showcase_shared::MAX_COORD);
        s.y = (s.y + dy).clamp(0, showcase_shared::MAX_COORD);
        self.changed();
        true
    }

    fn zoom_at(&mut self, factor: f64, x: f64, y: f64) {
        let zoom = (self.zoom * factor).clamp(0.25, 4.0);
        let (wx, wy) = self.world(x, y);
        self.zoom = zoom;
        self.pan = (x - wx * zoom, y - wy * zoom);
    }
}

/// A CSS custom property of the island, or `fallback`.
fn token(host: &Element, name: &str, fallback: &str) -> String {
    web_sys::window()
        .and_then(|w| {
            w.get_computed_style(host)
                .ok()
                .flatten()
        })
        .and_then(|s| s.get_property_value(name).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

impl Scene for Editor {
    fn draw(&mut self, g: &Graphics, frame: Frame) {
        let Graphics::TwoD(ctx) = g else { return };
        let (bg, line, text, brand) = (
            token(&self.host, "--z-surface", "#fff"),
            token(&self.host, "--z-border", "#ccc"),
            token(&self.host, "--z-text", "#111"),
            token(&self.host, "--z-brand", "#2563eb"),
        );
        ctx.set_fill_style_str(&bg);
        ctx.fill_rect(0.0, 0.0, frame.width, frame.height);
        let _ = ctx.save();
        let _ = ctx.translate(self.pan.0, self.pan.1);
        let _ = ctx.scale(self.zoom, self.zoom);
        ctx.set_font("14px system-ui, sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        for (i, s) in self.diagram.shapes.iter().enumerate() {
            shape_path(ctx, s);
            ctx.set_fill_style_str(&bg);
            ctx.fill();
            let chosen = self.selected == Some(i);
            ctx.set_stroke_style_str(if chosen { &brand } else { &line });
            ctx.set_line_width(if chosen { 3.0 } else { 1.5 } / self.zoom.max(0.5));
            ctx.stroke();
            ctx.set_fill_style_str(&text);
            let _ = ctx.fill_text_with_max_width(
                &s.label,
                f64::from(s.x) + f64::from(s.w) / 2.0,
                f64::from(s.y) + f64::from(s.h) / 2.0,
                f64::from(s.w - 12),
            );
        }
        let _ = ctx.restore();
    }

    fn input(&mut self, input: &Input, _frame: Frame) -> bool {
        match input {
            Input::Down {
                x, y, button: 0, ..
            } => {
                let hit = self.hit(*x, *y);
                if hit != self.selected {
                    self.select(hit);
                }
                self.grab = Some(match hit.and_then(|i| self.diagram.shapes.get(i)) {
                    Some(s) => {
                        let (wx, wy) = self.world(*x, *y);
                        Grab::Shape {
                            dx: wx - f64::from(s.x),
                            dy: wy - f64::from(s.y),
                        }
                    }
                    None => Grab::Pan {
                        x: *x - self.pan.0,
                        y: *y - self.pan.1,
                    },
                });
                true
            }
            Input::Move {
                x,
                y,
                pressed: true,
            } => match self.grab {
                Some(Grab::Shape { dx, dy }) => {
                    let (wx, wy) = self.world(*x, *y);
                    if let Some(s) = self
                        .selected
                        .and_then(|i| self.diagram.shapes.get_mut(i))
                    {
                        s.x = ((wx - dx).round() as i32).clamp(0, showcase_shared::MAX_COORD);
                        s.y = ((wy - dy).round() as i32).clamp(0, showcase_shared::MAX_COORD);
                    }
                    true
                }
                Some(Grab::Pan { x: ox, y: oy }) => {
                    self.pan = (*x - ox, *y - oy);
                    true
                }
                None => false,
            },
            Input::Up { .. } => {
                if matches!(self.grab.take(), Some(Grab::Shape { .. })) {
                    self.changed();
                }
                false
            }
            Input::Wheel { x, y, dy, .. } => {
                self.zoom_at((-dy * 0.0015).exp(), *x, *y);
                true
            }
            Input::Key {
                key,
                shift,
                ctrl: false,
                alt: false,
            } => {
                let by = if *shift { 10 } else { 1 };
                match key.as_str() {
                    "ArrowLeft" => self.nudge(-by, 0),
                    "ArrowRight" => self.nudge(by, 0),
                    "ArrowUp" => self.nudge(0, -by),
                    "ArrowDown" => self.nudge(0, by),
                    "]" => {
                        self.step(1);
                        true
                    }
                    "[" => {
                        self.step(-1);
                        true
                    }
                    "Delete" | "Backspace" => {
                        self.remove();
                        true
                    }
                    "+" | "=" => {
                        self.zoom_at(1.25, 0.0, 0.0);
                        true
                    }
                    "-" => {
                        self.zoom_at(0.8, 0.0, 0.0);
                        true
                    }
                    "0" => {
                        self.zoom = 1.0;
                        self.pan = (0.0, 0.0);
                        true
                    }
                    "Escape" if self.selected.is_some() => {
                        self.select(None);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

fn shape_path(ctx: &CanvasRenderingContext2d, s: &Shape) {
    let (x, y, w, h) = (
        f64::from(s.x),
        f64::from(s.y),
        f64::from(s.w),
        f64::from(s.h),
    );
    ctx.begin_path();
    match s.kind {
        ShapeKind::Box => ctx.rect(x, y, w, h),
        ShapeKind::Ellipse => {
            let _ = ctx.ellipse(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0, 0.0, 0.0, TAU);
        }
    }
}

thread_local! {
    /// Mounted editors by placeholder, so `unmount` can release them.
    static MOUNTED: RefCell<Vec<Mounted>> = const { RefCell::new(Vec::new()) };
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

fn on_click(target: &Element, f: impl FnMut() + 'static) -> Result<(), JsValue> {
    let cb = Closure::<dyn FnMut()>::new(f);
    target.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
    cb.forget();
    Ok(())
}

/// Called by `/_zeyn/islands.js` with the placeholder element.
#[wasm_bindgen]
pub fn mount(root: Element) -> Result<(), JsValue> {
    let island = Island::new(root.clone());
    let props: DiagramProps = island
        .props()
        .map_err(|e| JsValue::from_str(&e.message))?;
    let doc = root
        .owner_document()
        .ok_or("the island has no document")?;
    let texts = Rc::new(props.texts);

    let tools = make(&doc, "div", "z-row z-row-compact")?;
    tools.set_attribute("role", "toolbar")?;
    tools.set_attribute("aria-label", &texts.tools)?;
    let buttons: Vec<Element> = [
        &texts.add_box,
        &texts.add_ellipse,
        &texts.next,
        &texts.remove,
        &texts.save,
    ]
    .into_iter()
    .map(|label| {
        let b = make(&doc, "button", "z-button")?;
        b.set_text_content(Some(label));
        tools.append_child(&b)?;
        Ok(b)
    })
    .collect::<Result<_, JsValue>>()?;
    buttons[4].set_class_name("z-button z-primary");
    let status = make(&doc, "p", "")?;
    status.set_attribute("role", "status")?;
    status.set_attribute("data-testid", "diagram.status")?;
    let stage = make(&doc, "div", "")?;
    stage.set_attribute("style", "margin-block:8px")?;
    let heading = make(&doc, "h3", "")?;
    heading.set_text_content(Some(&texts.summary));
    let summary = make(&doc, "ul", "")?;
    summary.set_attribute("data-testid", "diagram.summary")?;
    let live_badge = make(&doc, "span", "")?;
    live_badge.set_attribute("data-testid", "diagram.live")?;
    tools.append_child(&live_badge)?;
    for part in [&tools, &stage, &status, &heading, &summary] {
        root.append_child(part)?;
    }

    let view = View {
        summary,
        status,
        texts: texts.clone(),
    };
    view.refresh(&props.diagram, None);
    let editor = Editor {
        diagram: props.diagram,
        selected: None,
        grab: None,
        zoom: 1.0,
        pan: (0.0, 0.0),
        host: root.clone(),
        view,
        on_edit: None,
    };
    let described = root
        .get_attribute("aria-describedby")
        .unwrap_or_default();
    let surface = Surface::new(
        &stage,
        editor,
        Prefer::TwoD,
        &texts.canvas,
        &described,
        props.height.clamp(200, 1200),
    )?;
    surface
        .canvas()
        .set_attribute("aria-roledescription", &texts.canvas)?;

    let actions: [Box<dyn Fn(&mut Editor)>; 4] = [
        Box::new(|e| e.add(ShapeKind::Box)),
        Box::new(|e| e.add(ShapeKind::Ellipse)),
        Box::new(|e| e.step(1)),
        Box::new(|e| e.remove()),
    ];
    for (button, action) in buttons.iter().zip(actions) {
        let surface = surface.clone();
        on_click(button, move || surface.update(|e| action(e)))?;
    }
    let save = surface.clone();
    let save_texts = texts.clone();
    on_click(&buttons[4], move || {
        let diagram = save.read(|e| e.diagram.clone());
        let (island, save, texts) = (island.clone(), save.clone(), save_texts.clone());
        zeyn_island::spawn(async move {
            let text = match island
                .call::<SaveDiagram>(&diagram)
                .await
            {
                Ok(saved) => texts
                    .saved
                    .replace("{count}", &saved.shapes.to_string()),
                Err(e) => e.message,
            };
            save.read(|e| e.view.say(&text));
        });
    })?;
    let channel = match props.live {
        Some(id) => Some(go_live(&surface, &id, &live_badge, &texts)?),
        None => None,
    };
    MOUNTED.with(|m| {
        m.borrow_mut().push(Mounted {
            root,
            surface,
            channel,
        })
    });
    Ok(())
}

/// Joins the live document `id`: local edits go out as Y updates, edits of others come in and
/// replace the diagram. Returns the channel, shared with the editor's `on_edit`.
fn go_live(
    surface: &Surface<Editor>,
    id: &str,
    badge: &Element,
    texts: &Rc<DiagramTexts>,
) -> Result<Rc<RefCell<Option<Channel>>>, JsValue> {
    let doc = Rc::new(crdt::Doc::new());
    let channel: Rc<RefCell<Option<Channel>>> = Rc::default();
    let (d, c) = (doc.clone(), channel.clone());
    surface.update(|e| {
        e.on_edit = Some(Rc::new(move |diagram: &Diagram| {
            if let (Some(update), Some(ch)) = (crdt::write(&d, diagram), c.borrow().as_ref()) {
                ch.send_update(&update);
            }
        }))
    });
    let (d, c) = (doc.clone(), channel.clone());
    let on_open = move || {
        if let Some(ch) = c.borrow().as_ref() {
            ch.send_update(&crdt::state(&d));
        }
    };
    let (d, s) = (doc.clone(), surface.clone());
    let on_update = move |update: &[u8]| {
        if crdt::apply(&d, update) {
            let next = crdt::read(&d);
            s.update(|e| e.replace(next));
        }
    };
    let (badge, texts) = (badge.clone(), texts.clone());
    let on_state = move |online: bool| {
        badge.set_text_content(Some(if online {
            &texts.live_on
        } else {
            &texts.live_off
        }))
    };
    *channel.borrow_mut() = Some(Channel::document(
        "diagram", id, on_open, on_update, on_state,
    )?);
    Ok(channel)
}

/// A mounted editor and its live channel.
struct Mounted {
    root: Element,
    surface: Surface<Editor>,
    channel: Option<Rc<RefCell<Option<Channel>>>>,
}

/// Called by the loader when the placeholder leaves the page: closes the socket, removes the
/// listeners and breaks the editor's link to the channel so everything can be freed.
#[wasm_bindgen]
pub fn unmount(root: Element) {
    MOUNTED.with(|m| {
        m.borrow_mut().retain(|item| {
            if item.root != root {
                return true;
            }
            if let Some(ch) = item
                .channel
                .as_ref()
                .and_then(|c| c.borrow_mut().take())
            {
                ch.close();
            }
            item.surface
                .update(|e| e.on_edit = None);
            item.surface.destroy();
            false
        })
    });
}
