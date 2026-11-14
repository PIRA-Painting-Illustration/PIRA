//! File › Export › Render Video where the platform's pickers save one file, not a folder of
//! frames (Android): every timeline frame goes into an MP4 through the platform's video encoder
//! ([`crate::Services::video_encoder`]), or into an animated GIF when the file is named `.gif`.

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest};

/// Longest side of a rendered MP4; tablet encoders reliably take up to 1920.
pub const MAX_SIDE: u32 = 1920;

/// Asks where to save, then renders the active document there.
pub fn ask(app: &mut PhotocraftApp) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document open")?;
    let document = st.doc.id;
    let ext = if app.services.video_encoder.is_some() { "mp4" } else { "gif" };
    let suggested = format!("{}.{ext}", stem(&st.doc.name));
    app.ask_file(FileDialogRequest::Save { suggested }, move |app, answer| match answer {
        FileDialogAnswer::SaveTo(path) => app.with_document(document, |app| render_to(app, &path)),
        _ => Err("the save dialog gave an unexpected answer".into()),
    })
}

/// Renders the active document's timeline to `path`: an animated GIF for a `.gif` name, an MP4
/// otherwise.
pub fn render_to(app: &mut PhotocraftApp, path: &str) -> Result<Value, String> {
    let bytes = if path.to_ascii_lowercase().ends_with(".gif") {
        let mut out = None;
        photocraft_engine::render_video_with(
            &app.session,
            &json!({"format": "gif"}),
            |_, b| {
                out = Some(b.to_vec());
                Ok(())
            },
            |_, _| true,
        )
        .map_err(|e| e.to_string())?;
        out.ok_or("nothing was rendered")?
    } else {
        let make = app.services.video_encoder.clone().ok_or("This device has no video encoder. Name the file .gif to save an animated GIF.")?;
        photocraft_engine::render_mp4(&app.session, MAX_SIDE, &make, |_, _| true)?
    };
    match app.services.write.as_mut() {
        Some(write) => write(path, &bytes)?,
        None => return Err("no file writer".into()),
    }
    app.ui.status = crate::i18n::fmt(tl!("Video rendered: {name}"), &[("name", crate::file_open::display_name(path).as_str())]);
    app.ui.status_error = false;
    Ok(json!({"file": path}))
}

/// The document's name without its extension.
fn stem(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).trim();
    if stem.is_empty() { "video".into() } else { stem.into() }
}

#[cfg(test)]
mod tests;
