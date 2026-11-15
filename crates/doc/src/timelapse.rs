//! File › Timelapse: a canvas's painting history as a series of small JPEG snapshots, one taken
//! shortly after each change, saved inside the `.pcraft` file and exported as a sped-up video.
//!
//! The log is not part of what Undo restores: undoing a stroke records the canvas as it is after
//! the undo, just as the stroke itself was recorded. Every version of a document in its history
//! therefore shares one log ([`TimelapseLog`] is a handle to it), and turning recording off
//! empties that one log for good.

use std::sync::{Arc, Mutex, MutexGuard};

/// Most snapshots one log holds. When it is full, every other snapshot is dropped, so a long
/// painting keeps its whole story at a coarser step instead of losing its start or its end.
pub const MAX_FRAMES: usize = 2000;

/// The longest side of a snapshot, in pixels: the largest export size.
pub const FRAME_SIDE: u32 = 1280;

/// One snapshot: the flattened canvas, scaled to fit [`FRAME_SIDE`], as a baseline JPEG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelapseFrame {
    pub width: u32,
    pub height: u32,
    pub jpeg: Arc<Vec<u8>>,
}

#[derive(Debug, Default)]
struct Data {
    recording: bool,
    frames: Vec<TimelapseFrame>,
}

/// A handle to a document's timelapse log, shared by every version of the document (see the
/// module notes). Cloning the handle shares the log; [`Self::detached`] copies it.
#[derive(Clone, Default)]
pub struct TimelapseLog(Arc<Mutex<Data>>);

impl TimelapseLog {
    /// A log holding `frames` (the newest [`MAX_FRAMES`] are kept, thinned as recording would).
    pub fn from_parts(recording: bool, frames: Vec<TimelapseFrame>) -> Self {
        let log = TimelapseLog::default();
        {
            let mut d = log.lock();
            d.recording = recording;
            for f in frames {
                push_thinned(&mut d.frames, f);
            }
        }
        log
    }

    fn lock(&self) -> MutexGuard<'_, Data> {
        // A panic while holding the lock can only leave the frame list mid-push, which is still a
        // valid list: carry on with it.
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn recording(&self) -> bool {
        self.lock().recording
    }

    /// Turns recording on or off. Off deletes every snapshot, for good.
    pub fn set_recording(&self, on: bool) {
        let mut d = self.lock();
        d.recording = on;
        if !on {
            d.frames = Vec::new();
        }
    }

    /// Adds a snapshot, if recording.
    pub fn push(&self, frame: TimelapseFrame) -> bool {
        let mut d = self.lock();
        if !d.recording {
            return false;
        }
        push_thinned(&mut d.frames, frame);
        true
    }

    pub fn len(&self) -> usize {
        self.lock().frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The snapshots, oldest first (shared, not copied).
    pub fn frames(&self) -> Vec<TimelapseFrame> {
        self.lock().frames.clone()
    }

    /// The newest snapshot.
    pub fn last(&self) -> Option<TimelapseFrame> {
        self.lock().frames.last().cloned()
    }

    /// Bytes of JPEG data held.
    pub fn bytes(&self) -> usize {
        self.lock().frames.iter().map(|f| f.jpeg.len()).sum()
    }

    /// A separate log with the same snapshots and setting, for a copy of the document that should
    /// record on its own from here.
    pub fn detached(&self) -> Self {
        let d = self.lock();
        TimelapseLog::from_parts(d.recording, d.frames.clone())
    }

    /// Whether two handles are the same log.
    pub fn same_log(&self, other: &TimelapseLog) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

fn push_thinned(frames: &mut Vec<TimelapseFrame>, frame: TimelapseFrame) {
    if frames.len() >= MAX_FRAMES {
        // Keep the first and every other one after it.
        let mut k = 0;
        frames.retain(|_| {
            k += 1;
            k % 2 == 1
        });
    }
    frames.push(frame);
}

impl std::fmt::Debug for TimelapseLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = self.lock();
        write!(f, "TimelapseLog {{ recording: {}, frames: {} }}", d.recording, d.frames.len())
    }
}

/// The log is a record of how the document was made, not part of its content: two documents
/// with the same layers and settings are equal whatever their logs hold.
impl PartialEq for TimelapseLog {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(n: u8) -> TimelapseFrame {
        TimelapseFrame { width: 2, height: 2, jpeg: Arc::new(vec![n]) }
    }

    #[test]
    fn records_only_while_on_and_off_deletes_everything() {
        let log = TimelapseLog::default();
        assert!(!log.push(frame(1)), "off by default");
        log.set_recording(true);
        assert!(log.push(frame(2)));
        assert_eq!(log.len(), 1);
        log.set_recording(false);
        assert!(log.is_empty());
        assert!(!log.recording());
    }

    #[test]
    fn every_version_of_a_document_shares_the_log() {
        let log = TimelapseLog::default();
        log.set_recording(true);
        let older = log.clone();
        log.push(frame(1));
        assert_eq!(older.len(), 1, "undoing to an older version keeps the snapshots");
        let copy = log.detached();
        copy.push(frame(2));
        assert_eq!((log.len(), copy.len()), (1, 2));
        assert!(!copy.same_log(&log));
    }

    #[test]
    fn a_full_log_thins_out_and_keeps_both_ends() {
        let log = TimelapseLog::default();
        log.set_recording(true);
        for i in 0..MAX_FRAMES + 1 {
            log.push(TimelapseFrame { width: 1, height: 1, jpeg: Arc::new((i as u32).to_le_bytes().to_vec()) });
        }
        let f = log.frames();
        assert!(f.len() <= MAX_FRAMES);
        assert_eq!(*f[0].jpeg, 0u32.to_le_bytes().to_vec(), "the first snapshot stays");
        assert_eq!(*f[f.len() - 1].jpeg, (MAX_FRAMES as u32).to_le_bytes().to_vec(), "the newest is last");
    }
}
