//! The gesture that ends a run on purpose.
//!
//! A long press rather than a rapid sequence, and the reason is this application's whole
//! subject: wiping a keyboard is a burst of short strikes landing on every key at once. A
//! three-strike combination is precisely what a cloth produces by accident. Three seconds
//! of one key held down is precisely what it cannot.
//!
//! The clock is read from a thread rather than from the tap callback, because the callback
//! only runs when an event arrives and holding a key still produces none once auto-repeat
//! is turned off. Polling makes the gesture independent of a setting the user may have
//! changed for unrelated reasons.

use crate::intercept::{KEY_DOWN, KEY_UP};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// The key to hold, and for how long. Escape is far from the letters, so a hand resting on
/// it is a decision rather than a slip.
const KEY: u16 = 0x35;
const HOLD: Duration = Duration::from_secs(3);

/// How often the hold is measured. Fine enough that the three seconds are not visibly four.
const TICK: Duration = Duration::from_millis(100);

/// The key is not currently down. A real timestamp can never collide with this.
const NOT_HELD: u64 = u64::MAX;

pub struct Gesture {
	started: Instant,
	pressed_at: Arc<AtomicU64>,
}

impl Gesture {
	/// Starts watching. Like the deadline, the watcher ends the process itself rather than
	/// reporting back: the tap dies with the process, so exiting is what releases the
	/// keyboard, and routing that through the interception path would make the way out
	/// depend on the thing it exists to escape from.
	pub fn watching() -> Self {
		let started = Instant::now();
		let pressed_at = Arc::new(AtomicU64::new(NOT_HELD));
		let watched = Arc::clone(&pressed_at);

		thread::spawn(move || {
			loop {
				thread::sleep(TICK);
				let since = watched.load(Ordering::Relaxed);
				if since == NOT_HELD {
					continue;
				}
				if started.elapsed().as_millis() as u64 - since >= HOLD.as_millis() as u64 {
					eprintln!("still: escape held for {}s, releasing the keyboard", HOLD.as_secs());
					process::exit(0);
				}
			}
		});

		Self { started, pressed_at }
	}

	/// Feeds one key event to the gesture.
	pub fn observe(&self, kind: u32, key: u16) {
		if key != KEY {
			return;
		}
		match kind {
			// Auto-repeat keeps sending key-downs while the key is held. Only the first of them
			// starts the clock, or the hold would never accumulate.
			KEY_DOWN => {
				let now = self.started.elapsed().as_millis() as u64;
				let _ =
					self.pressed_at.compare_exchange(NOT_HELD, now, Ordering::Relaxed, Ordering::Relaxed);
			}
			KEY_UP => self.pressed_at.store(NOT_HELD, Ordering::Relaxed),
			_ => {}
		}
	}
}
