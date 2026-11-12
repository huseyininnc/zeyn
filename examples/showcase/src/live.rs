//! Live updates over the page socket: a notice sent to the signed in user, and a job whose
//! progress arrives as events while it runs. Nothing on this screen polls.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeyn::prelude::*;
use zeyn::realtime::{Event, Socket, ZeynSocket};
use zeyn::ui::{OnEvent, OnEvents};

/// Sent to the user who pressed the button, on every page they have open.
#[derive(serde::Serialize, Event)]
#[event(name = "showcase.notice")]
pub struct Notice {
    pub text: String,
}

/// One step of the demo job, sent to the user who started it.
#[derive(serde::Serialize, Event)]
#[event(name = "showcase.job")]
pub struct JobStep {
    pub percent: u8,
}

/// How far the demo job of each user got, in percent.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<std::collections::HashMap<String, u8>>>);

impl Service for Jobs {}

impl Jobs {
    fn get(&self, user: &str) -> u8 {
        self.0
            .lock()
            .ok()
            .and_then(|m| m.get(user).copied())
            .unwrap_or(0)
    }

    fn set(&self, user: &str, percent: u8) {
        if let Ok(mut m) = self.0.lock() {
            if m.len() >= 1000 && !m.contains_key(user) {
                m.clear();
            }
            m.insert(user.to_string(), percent);
        }
    }
}

/// Declares the events of this screen on the app's socket.
pub fn events(live: ZeynSocket) -> ZeynSocket {
    live.event::<Notice>("showcase.islands.read")
        .event::<JobStep>("showcase.islands.read")
}

#[action(permission = "showcase.islands.read", ai = false)]
pub async fn send_notice(_: (), socket: Socket, ctx: Ctx) -> Result<()> {
    let user = ctx
        .identity()
        .user
        .clone()
        .unwrap_or_default();
    socket
        .to_user(&ctx, user)
        .emit(&Notice {
            text: ctx.t(&t!("live.notice_text")),
        })?;
    Ok(())
}

#[action(permission = "showcase.islands.read", ai = false)]
pub async fn start_job(_: (), socket: Socket, jobs: Jobs, ctx: Ctx) -> Result<()> {
    let user = ctx
        .identity()
        .user
        .clone()
        .unwrap_or_default();
    let tenant = ctx.tenant().map(|t| t.id().to_string());
    jobs.set(&user, 0);
    tokio::spawn(async move {
        for step in 1..=10u8 {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let percent = step * 10;
            jobs.set(&user, percent);
            let _ = socket
                .to_user_id(tenant.as_deref(), user.clone())
                .emit(&JobStep { percent });
        }
    });
    Ok(())
}

#[screen(route = "/live", permission = "showcase.islands.read")]
pub fn live_updates(_: (), jobs: Jobs, ctx: Ctx) -> Screen {
    let percent = jobs.get(
        ctx.identity()
            .user
            .as_deref()
            .unwrap_or(""),
    );
    Screen::new(t!("live.title")).body(
        Stack::new()
            .add(Paragraph::new(t!("live.intro")))
            .add(
                Panel::new(t!("live.notice"))
                    .add(Paragraph::new(t!("live.notice_help")))
                    .add(ActionButton::new(t!("live.send_notice"), send_notice, ()))
                    .on_event::<Notice>(OnEvent::Toast(t!("live.notice_text"))),
            )
            .add(
                Panel::new(t!("live.job"))
                    .add(Paragraph::new(t!("live.job_help")))
                    .add(Progress::new(t!("live.progress"), u64::from(percent), 100))
                    .add(ActionButton::new(t!("live.start_job"), start_job, ()))
                    .on_event::<JobStep>(OnEvent::Refresh),
            ),
    )
}
