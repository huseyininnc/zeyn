//! Streamed XLSX export: pages of typed values go to a blocking thread that writes them to a
//! constant memory worksheet; the zipped workbook is then sent to the response in chunks.

use crate::grid::ExportColumn;
use crate::value::Value;
use crate::xlsx::XlsxSheet;
use axum::body::Bytes;
use std::io::Write;
use tokio::sync::mpsc;

/// Size of the chunks the zipped workbook is sent in.
const CHUNK: usize = 64 * 1024;

type Out = mpsc::Sender<Result<Bytes, std::io::Error>>;

/// The writer side of one XLSX export.
pub(super) struct XlsxSink {
    rows: mpsc::Sender<Result<Vec<Vec<Value>>, ()>>,
    done: tokio::task::JoinHandle<Result<(), String>>,
}

impl XlsxSink {
    /// Starts the writer thread. At most two pages wait for it, so a slow disk slows the reads
    /// instead of filling memory.
    pub(super) fn open(columns: Vec<ExportColumn>, locale: String, out: Out) -> XlsxSink {
        let (rows, mut rx) = mpsc::channel::<Result<Vec<Vec<Value>>, ()>>(2);
        let done = tokio::task::spawn_blocking(move || {
            let result = (|| {
                let mut sheet = XlsxSheet::new(&columns, &locale).map_err(|e| e.to_string())?;
                loop {
                    match rx.blocking_recv() {
                        Some(Ok(page)) => sheet.write_rows(&page).map_err(|e| e.to_string())?,
                        Some(Err(())) => return Err("export failed".to_string()),
                        None => break,
                    }
                }
                let mut w = ChannelWriter { out: out.clone(), buf: Vec::with_capacity(CHUNK) };
                sheet.finish(&mut w).map_err(|e| e.to_string())?;
                w.flush().map_err(|e| e.to_string())
            })();
            if result.is_err() {
                let _ = out.blocking_send(Err(std::io::Error::other("export failed")));
            }
            result
        });
        XlsxSink { rows, done }
    }

    /// Hands one page to the writer. `false` when the writer stopped.
    pub(super) async fn send(&self, page: Vec<Vec<Value>>) -> bool {
        self.rows.send(Ok(page)).await.is_ok()
    }

    /// Ends the export: zips the workbook when `complete`, otherwise drops it, and waits for the
    /// writer.
    pub(super) async fn finish(self, complete: bool) -> Result<(), String> {
        if !complete {
            let _ = self.rows.send(Err(())).await;
        }
        drop(self.rows);
        self.done.await.map_err(|e| e.to_string())?
    }
}

/// Sends what is written to the response in chunks of [`CHUNK`] bytes.
struct ChannelWriter {
    out: Out,
    buf: Vec<u8>,
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK {
            self.flush()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = Bytes::from(std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK)));
        self.out.blocking_send(Ok(chunk)).map_err(|_| std::io::Error::other("the download was closed"))
    }
}
