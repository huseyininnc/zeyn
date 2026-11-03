//! E-mail notifications, behind the `email` Cargo feature.
//!
//! A [`Mailer`] renders a message from the i18n catalogs in the recipient's language when it is
//! queued and sends it later from a background job, so a request never waits for a mail server.
//! Failed messages are tried again with the job's backoff.
//!
//! ```ignore
//! let mailer = Mailer::from_config(&config)?;
//! App::new("People").mailer(mailer)
//!
//! #[action(permission = "people.leave.approve")]
//! pub async fn approve(req: TransitionRequest<LeaveId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<()> {
//!     ...
//!     mailer.send(&ctx, Mail::to(&leave.email).template("people.mail.moved").arg("name", leave.name))
//! }
//! ```
//!
//! Catalog entries for a template `t` are `t.subject`, `t.text` and optionally `t.html`. Arguments
//! fill `{name}` placeholders; in the HTML part every argument is escaped first.

use crate::action::{Error, Result};
use crate::ctx::{Ctx, Service};
use crate::i18n::{I18n, Text};
use crate::platform::{Config, Secret};
use crate::render::escape;
use lettre::message::{Mailbox, MultiPart};
use lettre::{AsyncTransport, Message};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Messages a mailer holds before `send` refuses more.
pub const MAX_QUEUED: usize = 10_000;
/// Times a message is tried before it is dropped (and logged).
pub const MAX_ATTEMPTS: u32 = 8;
/// Name of the background job that sends queued mail.
pub const MAIL_JOB: &str = "zeyn.mail";

/// One message to send, rendered from a catalog template.
#[derive(Clone, Debug)]
pub struct Mail {
    to: Vec<String>,
    template: String,
    locale: Option<String>,
    args: Vec<(&'static str, String)>,
}

impl Mail {
    /// A message to `address`.
    pub fn to(address: &str) -> Mail {
        Mail {
            to: vec![address.to_string()],
            template: String::new(),
            locale: None,
            args: Vec::new(),
        }
    }

    /// Adds another recipient.
    pub fn and(mut self, address: &str) -> Mail {
        self.to.push(address.to_string());
        self
    }

    /// The catalog prefix of the subject, text and HTML (`<template>.subject` and so on).
    pub fn template(mut self, template: impl Into<String>) -> Mail {
        self.template = template.into();
        self
    }

    /// The recipient's language. The sender's language unless set.
    pub fn locale(mut self, locale: &str) -> Mail {
        self.locale = Some(locale.to_string());
        self
    }

    /// Fills `{name}` in the subject, text and HTML. The HTML gets the value escaped.
    pub fn arg(mut self, name: &'static str, value: impl Into<String>) -> Mail {
        self.args.push((name, value.into()));
        self
    }

    /// A notice of the workflow move this call just made (see [`crate::Workflow::apply`]), with
    /// the built in `zeyn.mail.transition` template: `record` names the record for the reader.
    /// `NotFound` when the call made no move.
    pub fn transition(ctx: &Ctx, address: &str, record: &str) -> Result<Mail> {
        let moved = ctx
            .trail()
            .transition
            .ok_or(Error::NotFound)?;
        let by = ctx
            .identity()
            .user
            .clone()
            .unwrap_or_default();
        Ok(Mail::to(address)
            .template("zeyn.mail.transition")
            .locale(ctx.locale())
            .arg("record", record)
            .arg("move", moved.name)
            .arg("from", moved.from)
            .arg("to", moved.to)
            .arg("by", by)
            .arg("comment", moved.comment.unwrap_or_default()))
    }

    /// Subject, plain text and HTML in `locale`.
    pub fn render(&self, i18n: &I18n, locale: &str) -> (String, String, String) {
        let text = |part: &str, html: bool| {
            let mut t = Text::key(format!("{}.{part}", self.template));
            for (k, v) in &self.args {
                t = t.arg(k, if html { escape(v) } else { v.clone() });
            }
            i18n.text(locale, &t)
        };
        let subject: String = text("subject", false)
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let plain = text("text", false);
        let body = if i18n
            .lookup(locale, &format!("{}.html", self.template))
            .is_some()
        {
            text("html", true)
        } else {
            escape(&plain)
                .split("\n\n")
                .map(|p| format!("<p>{}</p>", p.replace('\n', "<br>")))
                .collect::<String>()
        };
        let html = format!(
            "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><title>{}</title></head><body>{body}</body></html>",
            escape(locale),
            escape(&subject)
        );
        (subject, plain, html)
    }
}

/// Where messages go.
#[derive(Clone)]
enum Transport {
    Smtp(lettre::AsyncSmtpTransport<lettre::Tokio1Executor>),
    /// Each message as an `.eml` file in a directory, for development and tests.
    File(std::path::PathBuf),
    /// Only a log line with the number of recipients and the size, never addresses or content.
    Log,
}

struct Queued {
    message: Message,
    attempts: u32,
}

struct Inner {
    from: Mailbox,
    transport: Transport,
    queue: Mutex<VecDeque<Queued>>,
    jobs: OnceLock<crate::jobs::Jobs>,
    retries: AtomicU32,
    backoff_ms: AtomicU64,
}

/// Sends e-mail through a background job. A service: register it with `App::mailer`, then inject
/// it into actions.
#[derive(Clone)]
pub struct Mailer(Arc<Inner>);

impl Service for Mailer {}

impl Mailer {
    /// Reads the configuration:
    ///
    /// - `mail.from`: the sender, such as `Northwind <no-reply@example.com>` (required)
    /// - `mail.transport`: `smtp`, `file` or `log` (default `log`)
    /// - `mail.smtp.host`, `mail.smtp.port`, `mail.smtp.tls` (`starttls`, `tls` or `none`; default
    ///   `starttls`), `mail.smtp.user`, `mail.smtp.password` (a secret)
    /// - `mail.file.dir`: the directory of the `file` transport
    pub fn from_config(config: &Config) -> Result<Mailer> {
        let from: Mailbox = config
            .require("mail.from")?
            .parse()
            .map_err(|_| Error::Internal("mail.from is not an e-mail address".into()))?;
        let transport = match config
            .get("mail.transport")
            .unwrap_or("log")
        {
            "log" => Transport::Log,
            "file" => Transport::File(config.require("mail.file.dir")?.into()),
            "smtp" => Transport::Smtp(smtp(config)?),
            other => {
                return Err(Error::Internal(format!(
                    "mail.transport `{other}` is not smtp, file or log"
                )));
            }
        };
        Ok(Mailer::new(from, transport))
    }

    /// A mailer that writes each message as an `.eml` file into `dir`.
    pub fn to_files(from: &str, dir: impl Into<std::path::PathBuf>) -> Result<Mailer> {
        let from = from
            .parse()
            .map_err(|_| Error::Internal("the sender is not an e-mail address".into()))?;
        Ok(Mailer::new(from, Transport::File(dir.into())))
    }

    fn new(from: Mailbox, transport: Transport) -> Mailer {
        Mailer(Arc::new(Inner {
            from,
            transport,
            queue: Mutex::new(VecDeque::new()),
            jobs: OnceLock::new(),
            retries: AtomicU32::new(5),
            backoff_ms: AtomicU64::new(10_000),
        }))
    }

    /// How often a failed delivery run is tried again and the first wait (doubled each time).
    /// Five times from ten seconds unless set. Call it before `App::mailer`.
    pub fn retry(self, times: u32, first_delay: Duration) -> Mailer {
        self.0
            .retries
            .store(times, Ordering::Relaxed);
        self.0.backoff_ms.store(
            first_delay
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
        self
    }

    /// Renders `mail` in the recipient's language (the user's unless set) and queues it. Returns
    /// at once; the mail job sends it. Fails when an address is invalid or the queue is full.
    pub fn send(&self, ctx: &Ctx, mail: Mail) -> Result<()> {
        let locale = mail
            .locale
            .clone()
            .unwrap_or_else(|| ctx.locale().to_string());
        let (subject, plain, html) = mail.render(ctx.i18n(), &locale);
        let mut builder = Message::builder()
            .from(self.0.from.clone())
            .subject(subject);
        for to in &mail.to {
            let mailbox: Mailbox = to
                .parse()
                .map_err(|_| Error::Internal("a recipient is not an e-mail address".into()))?;
            builder = builder.to(mailbox);
        }
        let message = builder
            .multipart(MultiPart::alternative_plain_html(plain, html))
            .map_err(|e| Error::Internal(format!("mail: {e}")))?;
        {
            let mut queue = self
                .0
                .queue
                .lock()
                .map_err(|_| Error::Internal("mail queue lock".into()))?;
            if queue.len() >= MAX_QUEUED {
                return Err(Error::Internal("the mail queue is full".into()));
            }
            queue.push_back(Queued {
                message,
                attempts: 0,
            });
        }
        if let Some(jobs) = self.0.jobs.get() {
            let _ = jobs.trigger(MAIL_JOB);
        }
        Ok(())
    }

    /// Messages waiting to be sent.
    pub fn queued(&self) -> usize {
        self.0
            .queue
            .lock()
            .map(|q| q.len())
            .unwrap_or(0)
    }

    /// Sends what is queued now. Messages that fail go back to the queue, and the run fails so
    /// the job tries again later; a message is dropped after [`MAX_ATTEMPTS`] tries.
    pub async fn flush(&self) -> Result<()> {
        let batch: Vec<Queued> = self
            .0
            .queue
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        let mut failed = 0usize;
        for mut item in batch {
            match self.deliver(&item.message).await {
                Ok(()) => {}
                Err(e) => {
                    item.attempts += 1;
                    if item.attempts >= MAX_ATTEMPTS {
                        tracing::error!("mail dropped after {} attempts: {e}", item.attempts);
                        continue;
                    }
                    tracing::warn!("mail not sent, will try again: {e}");
                    failed += 1;
                    if let Ok(mut q) = self.0.queue.lock() {
                        q.push_back(item);
                    }
                }
            }
        }
        if failed > 0 {
            Err(Error::Internal(format!("{failed} mail(s) not sent")))
        } else {
            Ok(())
        }
    }

    async fn deliver(&self, message: &Message) -> std::result::Result<(), String> {
        match &self.0.transport {
            Transport::Smtp(smtp) => smtp
                .send(message.clone())
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Transport::File(dir) => {
                let file = lettre::AsyncFileTransport::<lettre::Tokio1Executor>::new(dir);
                file.send(message.clone())
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Transport::Log => {
                let size = message.formatted().len();
                tracing::info!(
                    "mail (log transport): {} recipient(s), {size} bytes",
                    message.envelope().to().len()
                );
                Ok(())
            }
        }
    }

    pub(crate) fn attach(&self, jobs: crate::jobs::Jobs) {
        let _ = self.0.jobs.set(jobs);
    }

    pub(crate) fn schedule(&self) -> crate::jobs::Schedule {
        let backoff = Duration::from_millis(
            self.0
                .backoff_ms
                .load(Ordering::Relaxed),
        );
        crate::jobs::Schedule::every(Duration::from_secs(60))
            .retry(self.0.retries.load(Ordering::Relaxed), backoff)
    }
}

fn smtp(config: &Config) -> Result<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>> {
    type T = lettre::AsyncSmtpTransport<lettre::Tokio1Executor>;
    let host = config.require("mail.smtp.host")?;
    let builder = match config
        .get("mail.smtp.tls")
        .unwrap_or("starttls")
    {
        "starttls" => T::starttls_relay(host),
        "tls" => T::relay(host),
        "none" => Ok(T::builder_dangerous(host)),
        other => {
            return Err(Error::Internal(format!(
                "mail.smtp.tls `{other}` is not starttls, tls or none"
            )));
        }
    }
    .map_err(|e| Error::Internal(format!("mail.smtp.host: {e}")))?;
    let mut builder = builder.timeout(Some(Duration::from_secs(30)));
    if let Some(port) = config.get("mail.smtp.port") {
        builder = builder.port(
            port.parse()
                .map_err(|_| Error::Internal("mail.smtp.port is not a port number".into()))?,
        );
    }
    if let Some(user) = config.get("mail.smtp.user") {
        let password: Secret<String> = config.secret("mail.smtp.password")?;
        builder = builder.credentials(lettre::transport::smtp::authentication::Credentials::new(
            user.to_string(),
            password.expose().clone(),
        ));
    }
    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fill_arguments_and_escape_them_in_html() {
        let i18n = I18n::new("en")
            .catalog("en", r#"{ "m": { "subject": "Hello {name}\r\nBcc: x@y", "text": "Dear {name},\n\nbye", "html": "<p>Dear <b>{name}</b></p>" } }"#)
            .unwrap();
        let mail = Mail::to("a@b.c")
            .template("m")
            .arg("name", "<script>alert(1)</script>");
        let (subject, plain, html) = mail.render(&i18n, "en");
        assert_eq!(
            subject, "Hello <script>alert(1)</script>Bcc: x@y",
            "no line breaks reach the header"
        );
        assert_eq!(plain, "Dear <script>alert(1)</script>,\n\nbye");
        assert!(
            html.contains("<p>Dear <b>&lt;script&gt;alert(1)&lt;/script&gt;</b></p>"),
            "{html}"
        );
        assert!(!html.contains("<script>"));
        let (_, _, html) = Mail::to("a@b.c")
            .template("none")
            .arg("x", "<i>")
            .render(&I18n::new("en"), "en");
        assert!(html.contains("<body><p>none.text</p></body>"), "{html}");
    }
}
