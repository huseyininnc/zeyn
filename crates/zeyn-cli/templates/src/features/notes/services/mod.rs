//! Services of the feature: stores and clients that read and write data. Register each one in
//! `src/app.rs` with `.service(...)`.

mod note_store;
pub use note_store::NoteStore;
