//! `FileDrop`: a drop zone on its own, outside a form, that uploads files into a store and runs
//! an action for each stored file.

use super::{DataSource, dom_id};
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::files::{File, FileRules, MAX_FILE_NAME, clean_file_name, valid_file_id};
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

/// Runs the upload action `A`.
type Run = fn(Ctx, File) -> BoxFuture<Result<()>>;

fn run<A>(ctx: Ctx, file: File) -> BoxFuture<Result<()>>
where
    A: Action<Input = File>,
    A::Output: Outcome,
{
    Box::pin(async move {
        let output = A::call(ctx, file).await?;
        output.failure().map_or(Ok(()), Err)
    })
}

/// A drop zone that uploads files into the store of a [`FileRules`] and, after each file is
/// stored, runs an action with its [`File`], such as attaching it to a record.
///
/// Files come from the native file picker, a drop (folders too), or a paste while the zone has
/// focus. They upload with a progress bar, at most three at a time, and can be canceled, retried
/// and removed, like the upload field of a form. The file picker is the keyboard and single
/// pointer path. The store must be registered with `Feature::files`, which checks the upload
/// permission; the action checks its own. The action runs only for a file the same user
/// uploaded into the store in the same tenant.
///
/// ```ignore
/// #[action(permission = "docs.add")]
/// pub async fn add_document(file: File, docs: Documents) -> Result<()> {
///     docs.add(file).await
/// }
///
/// FileDrop::new(t!("docs.drop"), DOCS).on_upload(add_document)
/// ```
pub struct FileDrop {
    label: Text,
    rules: FileRules,
    many: bool,
    action: Option<(ActionMeta, Run)>,
}

impl FileDrop {
    /// A drop zone named `label` that takes several files of `rules`, up to its file count.
    pub fn new(label: impl Into<Text>, rules: FileRules) -> Self {
        FileDrop {
            label: label.into(),
            rules,
            many: true,
            action: None,
        }
    }

    /// Takes one file at a time; a new file replaces the one shown.
    pub fn single(mut self) -> Self {
        self.many = false;
        self
    }

    /// Runs `action` with each stored file. Without it the files are stored and listed only.
    pub fn on_upload<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = File>,
        A::Output: Outcome,
    {
        self.action = Some((A::META, run::<A>));
        self
    }

    /// The file the browser reports, if it has the shape the server gives: a server id, a name
    /// of at most [`MAX_FILE_NAME`] characters (cleaned again) and a type of the rules.
    fn check(&self, input: Json) -> Option<File> {
        let file: File = serde_json::from_value(input).ok()?;
        if !valid_file_id(&file.id) || file.name.chars().count() > MAX_FILE_NAME {
            return None;
        }
        let content_type = self
            .rules
            .types()
            .iter()
            .map(|t| t.mime())
            .find(|m| *m == file.content_type)?;
        Some(File {
            name: clean_file_name(&file.name),
            content_type: content_type.into(),
            ..file
        })
    }
}

impl From<FileDrop> for Node {
    fn from(d: FileDrop) -> Self {
        Node::new(d)
    }
}

impl DataSource for FileDrop {
    fn action(&self) -> ActionMeta {
        self.action
            .map(|(meta, _)| meta)
            .expect("source() offers a file drop only with an upload action")
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let file = self.check(input);
        let run = self.action.map(|(_, run)| run);
        let store = self.rules.store();
        Box::pin(async move {
            let (file, run) = file.zip(run).ok_or(Error::NotFound)?;
            let id = file.id.clone();
            crate::files::check_owned(&ctx, Some(store), vec![id.clone()]).await?;
            run(ctx.clone(), file).await?;
            crate::files::claim(&ctx, Some(store), vec![id]).await;
            Ok(json!({ "ok": true }))
        })
    }
}

impl Component for FileDrop {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "file-drop");
        out.push_str(&format!(
            r#"<div class="z-file-drop" data-zeyn-file-drop{}>"#,
            if self.action.is_some() {
                " data-save"
            } else {
                ""
            },
        ));
        crate::files::render_upload(
            out,
            r,
            crate::files::UploadField {
                label: &self.label,
                name: "files",
                required: false,
            },
            &id,
            &self.rules,
            self.many,
            &Json::Null,
        );
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "FileDrop",
            "store": self.rules.store(),
            "many": self.many,
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "upload" && self.action.is_some()).then_some(self as &dyn DataSource)
    }
}
