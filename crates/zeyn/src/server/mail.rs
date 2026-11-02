//! Wires a [`crate::Mailer`] into the app: the service for actions and the background job that
//! sends its queue.

use crate::mail::{MAIL_JOB, Mailer};

impl super::App {
    /// Sends e-mail with `mailer`. Actions inject it and queue messages with [`Mailer::send`];
    /// the `zeyn.mail` job sends them right away, tries failed ones again with backoff and looks
    /// at the queue every minute.
    pub fn mailer(self, mailer: Mailer) -> Self {
        let sender = mailer.clone();
        let app = self.job(MAIL_JOB, mailer.schedule(), move |_ctx| {
            let sender = sender.clone();
            async move { sender.flush().await }
        });
        if let Some(jobs) = app.services.get::<crate::jobs::Jobs>() {
            mailer.attach(jobs);
        }
        app.service(mailer)
    }
}
