//! Opening and saving files, which works differently on each platform: native dialogs on the
//! desktop, Android's document pickers (through the Activity, see `android.rs`) on a tablet.

use std::collections::VecDeque;

/// Why a file is being opened or saved; it comes back with the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Open a scene (replacing the current one).
    Open,
    /// Save the scene.
    Save,
    /// Open a reference photo to show behind the figures (and match poses to).
    Photo,
    /// Export a picture (the scene stays tied to its own file).
    Export,
}

/// Where a file lives: a path on the desktop, a document URI on Android.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location(pub String);

#[derive(Debug)]
pub enum Event {
    Opened { purpose: Purpose, name: String, bytes: Vec<u8>, location: Option<Location> },
    Saved { purpose: Purpose, name: String, location: Location },
    Failed(String),
    Cancelled,
}

pub trait Files {
    /// Asks for a file to open.
    fn open(&mut self, purpose: Purpose);
    /// Writes `bytes` to `to`, or asks where (suggesting `name`) when `to` is None.
    fn save(&mut self, purpose: Purpose, name: &str, bytes: Vec<u8>, to: Option<&Location>);
    /// The next answer, if one is ready.
    fn poll(&mut self) -> Option<Event>;
    /// True while waiting for the user to pick.
    fn busy(&self) -> bool {
        false
    }
}

/// The file's name, from a path or URI.
pub fn file_name(location: &str) -> String {
    location.rsplit(['/', '\\']).next().unwrap_or(location).to_string()
}

#[cfg(not(target_os = "android"))]
pub use desktop::Desktop;

#[cfg(not(target_os = "android"))]
mod desktop {
    use super::*;

    /// Native file dialogs (rfd), shown on the spot.
    #[derive(Default)]
    pub struct Desktop {
        events: VecDeque<Event>,
    }

    impl Files for Desktop {
        fn open(&mut self, purpose: Purpose) {
            let mut dialog = rfd::FileDialog::new();
            dialog = match purpose {
                Purpose::Photo => dialog.add_filter("Pictures", &["jpg", "jpeg", "png", "webp"]),
                _ => dialog.add_filter("PIRA Pose scene", &[crate::scene::FILE_EXTENSION]),
            };
            let Some(path) = dialog.pick_file() else {
                self.events.push_back(Event::Cancelled);
                return;
            };
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            self.events.push_back(match std::fs::read(&path) {
                Ok(bytes) => Event::Opened { purpose, name, bytes, location: Some(Location(path.to_string_lossy().into_owned())) },
                Err(e) => Event::Failed(format!("couldn't read {}: {e}", path.display())),
            });
        }

        fn save(&mut self, purpose: Purpose, name: &str, bytes: Vec<u8>, to: Option<&Location>) {
            let path = match to {
                Some(l) => std::path::PathBuf::from(&l.0),
                None => {
                    let ext = name.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
                    let Some(p) = rfd::FileDialog::new().set_file_name(name).add_filter(ext.to_uppercase(), &[ext.as_str()]).save_file() else {
                        self.events.push_back(Event::Cancelled);
                        return;
                    };
                    p
                }
            };
            let tmp = path.with_extension("saving");
            let result = std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, &path));
            self.events.push_back(match result {
                Ok(()) => Event::Saved {
                    purpose,
                    name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    location: Location(path.to_string_lossy().into_owned()),
                },
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    Event::Failed(format!("couldn't save {}: {e}", path.display()))
                }
            });
        }

        fn poll(&mut self) -> Option<Event> {
            self.events.pop_front()
        }
    }
}

/// For tests: answers nothing.
#[derive(Default)]
pub struct Nowhere {
    pub events: VecDeque<Event>,
}

impl Files for Nowhere {
    fn open(&mut self, _: Purpose) {
        self.events.push_back(Event::Cancelled);
    }
    fn save(&mut self, _: Purpose, _: &str, _: Vec<u8>, _: Option<&Location>) {
        self.events.push_back(Event::Cancelled);
    }
    fn poll(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
}
