//! Android: the entry point GameActivity starts, and file pickers through the Activity.
//!
//! The Storage Access Framework (document pickers, `content://` URIs) lives on the Java side
//! (`PoseActivity.java`). The two sides talk through small JSON files in the app's private
//! storage (`<files>/bridge`), as PIRA's Android app does (`apps/photocraft-android/src/bridge.rs`), so no JNI calls are needed:
//!
//! - `request.json`: Rust asks for something. `{"id", "kind": "open", "mime"}` shows a picker and
//!   copies the picked document in; `{"id", "kind": "save", "name", "mime", "data", "uri"?}`
//!   writes the file at `data` to the document `uri`, or to one the user picks when there's none.
//! - `answer.json`: the Activity's answer for that id: `{"id", "path"?, "name"?, "uri"?,
//!   "cancelled"?, "error"?}`.
//!
//! Each side writes a file under a temporary name and renames it, so the other never reads a
//! partial one.

use crate::files::{Event, Files, Location, Purpose};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use winit::platform::android::activity::AndroidApp;

/// android-activity looks the entry point up by name, hence `no_mangle` (the crate's only
/// `unsafe`; see `Cargo.toml`).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("PIRAPose"));
    // Panics end up in logcat rather than vanishing with the process.
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));
    if let Err(e) = run(app) {
        log::error!("PIRA Pose stopped: {e}");
    }
}

fn run(android: AndroidApp) -> Result<(), String> {
    let files_dir = android.internal_data_path().ok_or("no app storage")?;
    log::info!("PIRA Pose {} starting; data in {}", env!("CARGO_PKG_VERSION"), files_dir.display());
    let bridge = Bridge::new(&files_dir).map_err(|e| format!("couldn't make the file bridge: {e}"))?;
    let mut options = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, android_app: Some(android.clone()), ..Default::default() };
    // Vulkan first, GLES on devices without a usable Vulkan driver.
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        create.instance_descriptor.backends = eframe::wgpu::Backends::VULKAN | eframe::wgpu::Backends::GL;
    }
    let autosave = files_dir.join(format!("autosave.{}", crate::scene::FILE_EXTENSION));
    let workspace = files_dir.join("workspace.json");
    eframe::run_native(
        "PIRA Pose",
        options,
        Box::new(move |cc| Ok(Box::new(crate::Shell(crate::App::new(cc, Box::new(bridge), Some(autosave), Some(workspace)))))),
    )
    .map_err(|e| e.to_string())
}

#[derive(Deserialize, Default)]
struct Answer {
    id: u64,
    #[serde(default)]
    cancelled: bool,
    path: Option<String>,
    name: Option<String>,
    uri: Option<String>,
    error: Option<String>,
}

struct Pending {
    id: u64,
    purpose: Purpose,
    name: String,
}

pub struct Bridge {
    root: PathBuf,
    next_id: u64,
    pending: Option<Pending>,
}

impl Bridge {
    pub fn new(files_dir: &Path) -> std::io::Result<Self> {
        let root = files_dir.join("bridge");
        for dir in ["inbox", "out"] {
            let _ = std::fs::remove_dir_all(root.join(dir));
            std::fs::create_dir_all(root.join(dir))?;
        }
        for stale in ["request.json", "answer.json"] {
            let _ = std::fs::remove_file(root.join(stale));
        }
        // Ids only need to be unique within a run; start from the clock so a late answer from an
        // earlier run can't match.
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_millis() as u64);
        Ok(Bridge { root, next_id: seed, pending: None })
    }

    fn request(&mut self, purpose: Purpose, name: &str, body: serde_json::Value) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        let mut body = body;
        body["id"] = id.into();
        let tmp = self.root.join("request.json.tmp");
        let ok = std::fs::write(&tmp, body.to_string()).and_then(|()| std::fs::rename(&tmp, self.root.join("request.json")));
        match ok {
            Ok(()) => {
                self.pending = Some(Pending { id, purpose, name: name.to_string() });
                Some(id)
            }
            Err(e) => {
                log::error!("bridge request: {e}");
                None
            }
        }
    }
}

fn mime_of(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

impl Files for Bridge {
    fn open(&mut self, purpose: Purpose) {
        let mime = if purpose == Purpose::Photo { "image/*" } else { "*/*" };
        self.request(purpose, "", serde_json::json!({"kind": "open", "mime": mime}));
    }

    fn save(&mut self, purpose: Purpose, name: &str, bytes: Vec<u8>, to: Option<&Location>) {
        let data = self.root.join("out").join(format!("{}-{}", self.next_id + 1, name.replace(['/', '\\'], "_")));
        if let Err(e) = std::fs::write(&data, &bytes) {
            log::error!("save {name}: {e}");
            return;
        }
        let mut body = serde_json::json!({"kind": "save", "name": name, "mime": mime_of(name), "data": data.to_string_lossy()});
        if let Some(uri) = to {
            body["uri"] = uri.0.clone().into();
        }
        self.request(purpose, name, body);
    }

    fn poll(&mut self) -> Option<Event> {
        let pending = self.pending.as_ref()?;
        let path = self.root.join("answer.json");
        let text = std::fs::read_to_string(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        let a: Answer = serde_json::from_str(&text).unwrap_or_default();
        if a.id != pending.id {
            return None;
        }
        let Some(p) = self.pending.take() else { return None };
        if let Some(e) = a.error {
            return Some(Event::Failed(e));
        }
        if a.cancelled {
            return Some(Event::Cancelled);
        }
        let location = a.uri.map(Location);
        let name = a.name.unwrap_or(p.name);
        match p.purpose {
            Purpose::Save | Purpose::Export => Some(Event::Saved { purpose: p.purpose, name, location: location.unwrap_or(Location(String::new())) }),
            Purpose::Open | Purpose::Photo => {
                let file = a.path?;
                let bytes = std::fs::read(&file);
                let _ = std::fs::remove_file(&file);
                Some(match bytes {
                    Ok(bytes) => Event::Opened { purpose: p.purpose, name, bytes, location },
                    Err(e) => Event::Failed(format!("couldn't read {name}: {e}")),
                })
            }
        }
    }

    fn busy(&self) -> bool {
        self.pending.is_some()
    }
}
