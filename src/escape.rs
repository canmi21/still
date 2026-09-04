//! The gesture that ends a run on purpose.
//!
//! Unlike the deadline this one runs inside the interception path, so it only works while
//! everything else does. It exists for the ordinary case -- the user is finished and wants
//! the screen back now -- and not as a safety net.

use core_graphics::event::CGKeyCode;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Placeholder policy: three strikes of one key inside a window. The key, the count and the
/// window are all provisional; see the note in `Gesture::observe`.
const KEY: CGKeyCode = 0x35; // KeyCode::ESCAPE
const STRIKES: usize = 3;
const WINDOW: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct Gesture {
	recent: VecDeque<Instant>,
}

impl Gesture {
	/// Feeds one key-down to the gesture. Returns true when the run should end.
	///
	/// TODO(canmi): decide the real policy. What is here now is the obvious one and it is
	/// very likely wrong for this application: wiping a keyboard means pressing every key
	/// repeatedly, escape included, so a three-strike escape will fire within seconds of the
	/// user starting to clean. A gesture that survives that has to be something a cloth
	/// cannot produce -- a chord across two distant keys, a long hold, a key the cleaning
	/// hand does not reach, or the same key struck slowly rather than quickly.
	pub fn observe(&mut self, key: CGKeyCode) -> bool {
		if key != KEY {
			return false;
		}
		let now = Instant::now();
		self.recent.push_back(now);
		while self.recent.front().is_some_and(|first| now.duration_since(*first) > WINDOW) {
			self.recent.pop_front();
		}
		self.recent.len() >= STRIKES
	}
}
