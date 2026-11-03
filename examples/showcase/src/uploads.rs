//! File uploads: a support ticket with one contract and up to five attachments.
//!
//! Files go to an in memory store with a cap; the server checks the size, the type by content,
//! the permission and a scanner before it accepts a file. The save action looks every file up in
//! the store again, so a form cannot point at a file that was never uploaded.

use std::sync::LazyLock;
use zeyn::prelude::*;
use zeyn::{BoxFuture, FileMeta, FileScanner, FileStore, MediaReader, ScanVerdict};

/// Permission to upload and download ticket files.
const FILES: &str = "showcase.tickets.files";

/// What a ticket accepts: images and PDFs up to 2 MiB, at most five.
pub const TICKET_FILES: FileRules = FileRules::new("tickets")
    .max_bytes(2 * 1024 * 1024)
    .max_files(5)
    .accept(&[
        FileType::Png,
        FileType::Jpeg,
        FileType::Webp,
        FileType::Pdf,
        FileType::Csv,
    ]);

/// Uploaded ticket files, at most 32 MiB in all.
static STORE: LazyLock<MemoryStore> = LazyLock::new(|| MemoryStore::new(32 * 1024 * 1024));

/// A support ticket with files.
#[model]
pub struct Ticket {
    #[rule(required, max_len = 80)]
    pub title: String,
    pub contract: Option<File>,
    pub attachments: Vec<File>,
}

/// Refuses files whose name says they are a test virus, like the EICAR test file convention.
struct NameScanner;

impl FileScanner for NameScanner {
    fn scan(&self, meta: FileMeta, _file: Box<dyn MediaReader>) -> BoxFuture<Result<ScanVerdict>> {
        let refused = meta
            .name
            .to_lowercase()
            .contains("eicar");
        Box::pin(async move {
            Ok(if refused {
                ScanVerdict::Refused("test signature in the name".into())
            } else {
                ScanVerdict::Clean
            })
        })
    }
}

/// Saves a ticket after checking that every file is in the store.
#[action(permission = "showcase.tickets.files", audit)]
pub async fn save_ticket(ticket: Valid<Ticket>) -> Result<usize> {
    let ticket = ticket.into_inner();
    let files: Vec<&File> = ticket
        .contract
        .iter()
        .chain(ticket.attachments.iter())
        .collect();
    for f in &files {
        STORE
            .meta(f.id.clone())
            .await
            .map_err(|_| Error::NotFound)?;
    }
    Ok(files.len())
}

#[screen(route = "/uploads", permission = "showcase.tickets.files")]
pub fn uploads() -> Screen {
    Screen::new(t!("uploads.title")).body(
        Card::new()
            .title(t!("uploads.card"))
            .add(
                Form::of(Ticket::default())
                    .field(Ticket::title)
                    .file(Ticket::contract, TICKET_FILES.max_files(1))
                    .file(Ticket::attachments, TICKET_FILES)
                    .submit(t!("uploads.save"), save_ticket)
                    .on_success(|n, ui| {
                        if let Ok(n) = n {
                            ui.toast(t!("uploads.saved", n = n));
                        }
                    }),
            ),
    )
}

pub fn feature() -> Feature {
    Feature::new("uploads")
        .files(TICKET_FILES, FILES, STORE.clone())
        .scan_files(NameScanner)
        .screen(uploads)
        .menu(|m| {
            m.group(t!("menu.uploads"), Icon::Box)
                .item(t!("uploads.title"), uploads)
        })
}
