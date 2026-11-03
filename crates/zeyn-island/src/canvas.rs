//! Drawing surface for canvas islands (feature `canvas`).
//!
//! [`Surface`] puts a `<canvas>` into the island, keeps its backing store at the device pixel
//! ratio, turns pointer, wheel and keyboard events into [`Input`], and draws a [`Scene`] in a
//! `requestAnimationFrame` callback only when something changed. Nothing runs while the picture
//! is still: no timer, no loop.
//!
//! Graphics: [`Graphics::Gl`] (WebGL 2) when asked for and available, otherwise
//! [`Graphics::TwoD`]. [`webgpu_available`] tells whether the browser offers WebGPU, for scenes
//! that set it up themselves on [`Surface::canvas`].

use std::cell::{Cell, RefCell};

/// A DOM event name and the closure that handles it, kept so it can be removed.
type Listener = (&'static str, Closure<dyn FnMut(Event)>);
/// The resize observer of a canvas and its callback.
type Observer = (ResizeObserver, Closure<dyn FnMut()>);
use std::rc::{Rc, Weak};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    AddEventListenerOptions, CanvasRenderingContext2d, Element, Event, HtmlCanvasElement,
    KeyboardEvent, PointerEvent, ResizeObserver, WebGl2RenderingContext, WheelEvent,
};

/// The context a scene draws with.
pub enum Graphics {
    /// 2D context, already scaled so that one unit is one CSS pixel.
    TwoD(CanvasRenderingContext2d),
    /// WebGL 2; the viewport is set to the backing store size before each draw.
    Gl(WebGl2RenderingContext),
}

/// Which context to try first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefer {
    TwoD,
    /// WebGL 2, falling back to 2D when the browser or the GPU refuses it.
    Gl,
}

/// Size of the drawing area.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// CSS pixels.
    pub width: f64,
    pub height: f64,
    /// Device pixels per CSS pixel.
    pub ratio: f64,
}

/// What happened on the canvas. Coordinates are CSS pixels from the canvas' top left corner.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Down {
        x: f64,
        y: f64,
        button: i16,
        shift: bool,
    },
    Move {
        x: f64,
        y: f64,
        pressed: bool,
    },
    Up {
        x: f64,
        y: f64,
    },
    /// `dy` is positive when scrolling down; `ctrl` is set for pinch zoom on touchpads.
    Wheel {
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        ctrl: bool,
    },
    /// `key` is `KeyboardEvent.key` (`ArrowLeft`, `Delete`, `a`).
    Key {
        key: String,
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
}

/// A picture and how it reacts.
pub trait Scene: 'static {
    fn draw(&mut self, g: &Graphics, frame: Frame);

    /// Handles an input. `true` means it was used: the browser's default action is prevented and
    /// the scene is drawn again. Return `false` for keys the page needs, such as Tab.
    fn input(&mut self, input: &Input, frame: Frame) -> bool;

    /// Keeps drawing on the next frame too (an animation in progress).
    fn animating(&self) -> bool {
        false
    }
}

/// Whether `navigator.gpu` exists.
pub fn webgpu_available() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w.navigator(), &JsValue::from_str("gpu")).ok())
        .is_some_and(|gpu| !gpu.is_undefined() && !gpu.is_null())
}

struct Shared<S: Scene> {
    canvas: HtmlCanvasElement,
    graphics: Graphics,
    scene: RefCell<S>,
    frame: Cell<Frame>,
    pending: Cell<bool>,
    raf: RefCell<Option<Closure<dyn FnMut()>>>,
    listeners: RefCell<Vec<Listener>>,
    observer: RefCell<Option<Observer>>,
}

/// A canvas inside an island, drawing one scene. Cloning gives another handle to it.
pub struct Surface<S: Scene> {
    shared: Rc<Shared<S>>,
}

impl<S: Scene> Clone for Surface<S> {
    fn clone(&self) -> Self {
        Surface {
            shared: self.shared.clone(),
        }
    }
}

impl<S: Scene> Surface<S> {
    /// Creates the canvas inside `parent`. `label` is its accessible name; `described_by` the ids
    /// of elements that describe it (the island's description and text summary). The canvas
    /// takes focus with Tab and is `height` CSS pixels high, as wide as its parent.
    pub fn new(
        parent: &Element,
        scene: S,
        prefer: Prefer,
        label: &str,
        described_by: &str,
        height: u32,
    ) -> Result<Self, JsValue> {
        let doc = parent
            .owner_document()
            .ok_or("no document")?;
        let canvas: HtmlCanvasElement = doc
            .create_element("canvas")?
            .dyn_into()?;
        canvas.set_tab_index(0);
        canvas.set_attribute("role", "application")?;
        canvas.set_attribute("aria-label", label)?;
        if !described_by.is_empty() {
            canvas.set_attribute("aria-describedby", described_by)?;
        }
        let style = canvas.style();
        style.set_property("display", "block")?;
        style.set_property("width", "100%")?;
        style.set_property("height", &format!("{height}px"))?;
        style.set_property("touch-action", "none")?;
        parent.append_child(&canvas)?;
        let graphics = context(&canvas, prefer)?;
        let shared = Rc::new(Shared {
            canvas,
            graphics,
            scene: RefCell::new(scene),
            frame: Cell::new(Frame::default()),
            pending: Cell::new(false),
            raf: RefCell::new(None),
            listeners: RefCell::new(Vec::new()),
            observer: RefCell::new(None),
        });
        let surface = Surface { shared };
        surface.wire()?;
        surface.redraw();
        Ok(surface)
    }

    /// The `<canvas>` element.
    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.shared.canvas
    }

    /// Whether WebGL 2 was used.
    pub fn is_gl(&self) -> bool {
        matches!(self.shared.graphics, Graphics::Gl(_))
    }

    /// Changes the scene and draws it on the next frame.
    pub fn update<R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        let r = f(&mut self.shared.scene.borrow_mut());
        self.redraw();
        r
    }

    /// Reads the scene.
    pub fn read<R>(&self, f: impl FnOnce(&S) -> R) -> R {
        f(&self.shared.scene.borrow())
    }

    /// Draws on the next animation frame. Several calls before it run one draw.
    pub fn redraw(&self) {
        request(&self.shared);
    }

    /// Removes the listeners and the canvas. Call it from the island's `unmount`.
    pub fn destroy(&self) {
        for (kind, cb) in self
            .shared
            .listeners
            .borrow_mut()
            .drain(..)
        {
            let _ = self
                .shared
                .canvas
                .remove_event_listener_with_callback(kind, cb.as_ref().unchecked_ref());
        }
        if let Some((observer, _)) = self.shared.observer.borrow_mut().take() {
            observer.disconnect();
        }
        self.shared.raf.borrow_mut().take();
        self.shared.canvas.remove();
    }

    fn wire(&self) -> Result<(), JsValue> {
        let weak = Rc::downgrade(&self.shared);
        *self.shared.raf.borrow_mut() = Some(Closure::new({
            let weak = weak.clone();
            move || {
                if let Some(s) = weak.upgrade() {
                    paint(&s);
                }
            }
        }));
        let resized = Closure::<dyn FnMut()>::new({
            let weak = weak.clone();
            move || {
                if let Some(s) = weak.upgrade() {
                    request(&s);
                }
            }
        });
        let observer = ResizeObserver::new(resized.as_ref().unchecked_ref())?;
        observer.observe(&self.shared.canvas);
        *self.shared.observer.borrow_mut() = Some((observer, resized));
        for kind in [
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "wheel",
            "keydown",
        ] {
            let weak: Weak<Shared<S>> = weak.clone();
            let cb = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
                if let Some(s) = weak.upgrade() {
                    handle(&s, kind, &e);
                }
            });
            let options = AddEventListenerOptions::new();
            options.set_passive(false);
            self.shared
                .canvas
                .add_event_listener_with_callback_and_add_event_listener_options(
                    kind,
                    cb.as_ref().unchecked_ref(),
                    &options,
                )?;
            self.shared
                .listeners
                .borrow_mut()
                .push((kind, cb));
        }
        Ok(())
    }
}

fn context(canvas: &HtmlCanvasElement, prefer: Prefer) -> Result<Graphics, JsValue> {
    if prefer == Prefer::Gl
        && let Some(gl) = canvas
            .get_context("webgl2")
            .ok()
            .flatten()
            .and_then(|c| {
                c.dyn_into::<WebGl2RenderingContext>()
                    .ok()
            })
    {
        return Ok(Graphics::Gl(gl));
    }
    let ctx = canvas
        .get_context("2d")?
        .ok_or("no 2d context")?
        .dyn_into::<CanvasRenderingContext2d>()?;
    Ok(Graphics::TwoD(ctx))
}

fn request<S: Scene>(s: &Shared<S>) {
    if s.pending.replace(true) {
        return;
    }
    let scheduled = web_sys::window().zip(s.raf.borrow().as_ref().map(|c| {
        c.as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone()
    }));
    match scheduled {
        Some((w, f)) if w.request_animation_frame(&f).is_ok() => {}
        _ => s.pending.set(false),
    }
}

/// Matches the backing store to the element size and device pixel ratio, then draws.
fn paint<S: Scene>(s: &Shared<S>) {
    s.pending.set(false);
    if !s.canvas.is_connected() {
        return;
    }
    let ratio = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0)
        .max(1.0);
    let (width, height) = (
        f64::from(s.canvas.client_width()),
        f64::from(s.canvas.client_height()),
    );
    let (pw, ph) = (
        (width * ratio).round() as u32,
        (height * ratio).round() as u32,
    );
    if s.canvas.width() != pw || s.canvas.height() != ph {
        s.canvas.set_width(pw);
        s.canvas.set_height(ph);
    }
    let frame = Frame {
        width,
        height,
        ratio,
    };
    s.frame.set(frame);
    match &s.graphics {
        Graphics::TwoD(ctx) => {
            let _ = ctx.set_transform(ratio, 0.0, 0.0, ratio, 0.0, 0.0);
            ctx.clear_rect(0.0, 0.0, width, height);
        }
        Graphics::Gl(gl) => gl.viewport(0, 0, pw as i32, ph as i32),
    }
    let again = {
        let mut scene = s.scene.borrow_mut();
        scene.draw(&s.graphics, frame);
        scene.animating()
    };
    if again {
        request(s);
    }
}

fn handle<S: Scene>(s: &Shared<S>, kind: &str, e: &Event) {
    let input = match kind {
        "pointerdown" | "pointermove" | "pointerup" | "pointercancel" => {
            let Some(p) = e.dyn_ref::<PointerEvent>() else {
                return;
            };
            let (x, y) = (f64::from(p.offset_x()), f64::from(p.offset_y()));
            match kind {
                "pointerdown" => {
                    let _ = s
                        .canvas
                        .set_pointer_capture(p.pointer_id());
                    let _ = s.canvas.focus();
                    Input::Down {
                        x,
                        y,
                        button: p.button(),
                        shift: p.shift_key(),
                    }
                }
                "pointermove" => Input::Move {
                    x,
                    y,
                    pressed: p.buttons() & 1 == 1,
                },
                _ => Input::Up { x, y },
            }
        }
        "wheel" => {
            let Some(w) = e.dyn_ref::<WheelEvent>() else {
                return;
            };
            let scale = match w.delta_mode() {
                1 => 16.0,
                2 => f64::from(s.canvas.client_height()),
                _ => 1.0,
            };
            Input::Wheel {
                x: f64::from(w.offset_x()),
                y: f64::from(w.offset_y()),
                dx: w.delta_x() * scale,
                dy: w.delta_y() * scale,
                ctrl: w.ctrl_key(),
            }
        }
        _ => {
            let Some(k) = e.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            Input::Key {
                key: k.key(),
                shift: k.shift_key(),
                ctrl: k.ctrl_key() || k.meta_key(),
                alt: k.alt_key(),
            }
        }
    };
    let used = s
        .scene
        .borrow_mut()
        .input(&input, s.frame.get());
    if used {
        e.prevent_default();
        request(s);
    }
}
