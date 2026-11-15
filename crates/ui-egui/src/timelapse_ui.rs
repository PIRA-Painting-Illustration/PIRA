//! File › Timelapse in the app: snapshots of recording canvases taken off the UI thread, and
//! the Export Timelapse window (length, size and shape), which asks
//! where to save, renders and encodes on a worker with a progress bar, and writes the `.mp4`.
//! The engine does the work (`photocraft_engine::timelapse_cmds`); the platform supplies the
//! video encoder ([`crate::Services::video_encoder`]).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use photocraft_engine::timelapse_cmds::{self as tlc, Aspect, ExportOptions};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest};

/// The Export Timelapse window's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportDialog {
    pub opts: ExportOptions,
    running: Option<Running>,
}

type Outcome = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

/// An export on its worker thread.
#[derive(Clone)]
struct Running {
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    outcome: Outcome,
    path: String,
}

impl PartialEq for Running {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancel, &other.cancel)
    }
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Running {{ {} / {} → {} }}", self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed), self.path)
    }
}

/// File › Timelapse › Export Timelapse…: opens the window.
pub fn open(app: &mut PhotocraftApp) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document open")?;
    if app.services.video_encoder.is_none() {
        return Err(tl!("Timelapse export isn't available on this device yet.").into());
    }
    if st.doc.timelapse.is_empty() {
        return Err(tl!("This canvas has no timelapse yet. Turn on File › Timelapse › Record Timelapse and paint.").into());
    }
    if app.ui.timelapse_export.is_none() {
        app.ui.timelapse_export = Some(ExportDialog::default());
    }
    Ok(json!({"open": true}))
}

/// Every frame: start due snapshots, keep frames coming while one is due, draw the window and
/// finish an export that has completed.
pub fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    for job in app.session.timelapse_jobs() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if std::thread::Builder::new().name("timelapse snapshot".into()).spawn(move || job.run()).is_err() {
                log::warn!("couldn't start a timelapse snapshot");
            }
        }
        #[cfg(target_arch = "wasm32")]
        job.run();
    }
    if app.session.timelapse_waiting() {
        ctx.request_repaint_after(tlc::MIN_GAP);
    }
    finish(app);
    if app.ui.timelapse_export.is_some() {
        window(app, ctx);
    }
}

fn window(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(st) = app.session.active() else {
        app.ui.timelapse_export = None;
        return;
    };
    let frames = st.doc.timelapse.len();
    let canvas = (st.doc.size.width, st.doc.size.height);
    let Some(mut d) = app.ui.timelapse_export.clone() else { return };
    let mut open = true;
    let mut export = false;
    let mut cancel = false;
    egui::Window::new(tl!("Export Timelapse")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).open(&mut open).show(
        ctx,
        |ui| {
            if let Some(r) = &d.running {
                let (done, total) = (r.done.load(Ordering::Relaxed), r.total.load(Ordering::Relaxed).max(1));
                ui.label(tl!("Making the video…"));
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage().desired_width(320.0));
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                if ui.button(tl!("Cancel")).clicked() {
                    r.cancel.store(true, Ordering::Relaxed);
                }
                return;
            }
            ui.weak(crate::i18n::fmt(tl!("{n} snapshots recorded."), &[("n", frames.to_string().as_str())]));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(tl!("Length"));
                for s in tlc::LENGTHS {
                    if crate::widgets::pill_tab(ui, &format!("{s} s"), d.opts.seconds == Some(s)).clicked() {
                        d.opts.seconds = Some(s);
                    }
                }
                if crate::widgets::pill_tab(ui, tl!("All"), d.opts.seconds.is_none()).on_hover_text(tl!("Every snapshot once")).clicked() {
                    d.opts.seconds = None;
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tl!("Size"));
                for s in tlc::SIZES {
                    if crate::widgets::pill_tab(ui, &format!("{s} px"), d.opts.side == s).clicked() {
                        d.opts.side = s;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tl!("Shape"));
                for a in Aspect::ALL {
                    if crate::widgets::pill_tab(ui, a.label(), d.opts.aspect == a).clicked() {
                        d.opts.aspect = a;
                    }
                }
            });
            ui.add_space(6.0);
            let (w, h) = tlc::output_size(canvas, d.opts.side, d.opts.aspect);
            let body = d.opts.seconds.map_or(frames as f32 / tlc::FPS as f32, |s| s as f32);
            let secs = body + tlc::HOLD_SECONDS as f32;
            ui.weak(format!("{w} × {h} px · {secs:.0} s · MP4"));
            if d.opts.aspect != Aspect::Original {
                ui.weak(tl!("Black fills the space around the canvas."));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                export = crate::widgets::primary_button(ui, tl!("Export…"), 96.0).clicked();
                cancel = ui.button(tl!("Cancel")).clicked();
            });
        },
    );
    if (!open || cancel) && d.running.is_none() {
        app.ui.timelapse_export = None;
        return;
    }
    if let Some(r) = d.running.as_ref().filter(|_| !open) {
        r.cancel.store(true, Ordering::Relaxed);
    }
    app.ui.timelapse_export = Some(d.clone());
    if export {
        let stem = file_stem(&st_name(app));
        let r = app.ask_file(FileDialogRequest::Save { suggested: format!("{stem} timelapse.mp4") }, move |app, answer| match answer {
            FileDialogAnswer::SaveTo(path) => start(app, d.opts, path),
            _ => Err("the save dialog gave an unexpected answer".into()),
        });
        if let Err(e) = r
            && e != crate::file_dialog::CANCELLED
        {
            report(app, e, true);
        }
    }
}

fn st_name(app: &PhotocraftApp) -> String {
    app.session.active().map_or_else(|| "Untitled".into(), |st| st.doc.name.clone())
}

fn file_stem(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    if stem.is_empty() { "Untitled".into() } else { stem.into() }
}

/// Starts encoding to `path` on a worker thread.
fn start(app: &mut PhotocraftApp, opts: ExportOptions, path: String) -> Result<Value, String> {
    let make = app.services.video_encoder.clone().ok_or("no video encoder")?;
    let doc = app.session.active().ok_or("no document open")?.doc.clone();
    let r = Running { done: Arc::default(), total: Arc::new(AtomicUsize::new(1)), cancel: Arc::default(), outcome: Arc::default(), path: path.clone() };
    let w = r.clone();
    let work = move || {
        let result = tlc::export(&doc, &opts, &make, |done, total| {
            w.done.store(done, Ordering::Relaxed);
            w.total.store(total, Ordering::Relaxed);
            !w.cancel.load(Ordering::Relaxed)
        });
        if let Ok(mut slot) = w.outcome.lock() {
            *slot = Some(result);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new().name("timelapse export".into()).spawn(work).map_err(|e| format!("couldn't start the export: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    work();
    let d = app.ui.timelapse_export.get_or_insert_with(ExportDialog::default);
    d.opts = opts;
    d.running = Some(r);
    Ok(json!({"exporting": path}))
}

/// Writes a finished export and closes the window, or reports why it failed.
fn finish(app: &mut PhotocraftApp) {
    let Some(r) = app.ui.timelapse_export.as_ref().and_then(|d| d.running.clone()) else { return };
    let Some(result) = r.outcome.lock().ok().and_then(|mut s| s.take()) else { return };
    app.ui.timelapse_export = None;
    match result {
        Ok(bytes) => {
            let written = match app.services.write.as_mut() {
                Some(write) => write(&r.path, &bytes),
                None => Err("no file writer".into()),
            };
            match written {
                Ok(()) => report(app, crate::i18n::fmt(tl!("Timelapse saved: {name}"), &[("name", crate::file_open::display_name(&r.path).as_str())]), false),
                Err(e) => report(app, e, true),
            }
        }
        Err(e) if e == "cancelled" => report(app, tl!("Timelapse export cancelled").into(), false),
        Err(e) => report(app, e, true),
    }
}

fn report(app: &mut PhotocraftApp, message: String, error: bool) {
    app.ui.status = message;
    app.ui.status_error = error;
}

#[cfg(test)]
mod tests;
