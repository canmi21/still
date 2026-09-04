//! The gestures that end a run on purpose.
//!
//! Two of them, because they fail in opposite directions. A long press is the one a cloth
//! cannot forge -- wiping a keyboard lands short strikes on every key at once and never
//! rests on one for seconds -- but it costs three seconds every time. A short burst is
//! instant, and is exactly the accident a cloth produces. Keeping both means the burst is
//! there for a user with a hand free, and the hold is there for the case the burst is
//! eventually turned off in.
//!
//! Auto-repeat is what makes the two fit together. Holding a key generates a stream of
//! key-downs a few milliseconds apart, so without telling a repeat from a real strike the
//! burst would always fire first and the hold could never be reached.

use crate::intercept::{KEY_DOWN, KEY_UP};
use std::collections::VecDeque;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The key both gestures are spelled with. Escape is far from the letters, so a hand resting
/// on it is a decision rather than a slip.
const KEY: u16 = 0x35;

/// Hold this long, or strike this many times inside this window.
const HOLD: Duration = Duration::from_secs(3);
const STRIKES: usize = 3;
const WINDOW: Duration = Duration::from_millis(1500);

/// How often the hold is measured. Fine enough that the three seconds are not visibly four.
const TICK: Duration = Duration::from_millis(100);

/// The key is not currently down. A real timestamp can never collide with this.
const NOT_HELD: u64 = u64::MAX;

pub struct Gesture {
	started: Instant,
	pressed_at: Arc<AtomicU64>,
	recent: Mutex<VecDeque<Instant>>,
}

impl Gesture {
	/// Starts watching for the hold. Like the deadline, both gestures end the process
	/// themselves rather than reporting back: the tap dies with the process, so exiting is
	/// what releases the keyboard, and routing that through the interception path would make
	/// the way out depend on the thing it exists to escape from.
	///
	/// The hold is measured from a thread rather than accumulated in the tap callback,
	/// because a held key stops producing events entirely once key repeat is turned off in
	/// system settings. Polling keeps it working regardless.
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
					release(&format!("escape held for {}s", HOLD.as_secs()));
				}
			}
		});

		Self { started, pressed_at, recent: Mutex::new(VecDeque::new()) }
	}

	/// Feeds one key event to both gestures. `repeat` marks a key-down the system generated
	/// from a key that is still held, rather than one the user produced.
	pub fn observe(&self, kind: u32, key: u16, repeat: bool) {
		if key != KEY {
			return;
		}
		match kind {
			KEY_DOWN => {
				// Only the first key-down starts the hold, or a repeat would reset the clock it is
				// supposed to be advancing.
				let now = self.started.elapsed().as_millis() as u64;
				let _ =
					self.pressed_at.compare_exchange(NOT_HELD, now, Ordering::Relaxed, Ordering::Relaxed);
				if !repeat {
					self.strike();
				}
			}
			KEY_UP => self.pressed_at.store(NOT_HELD, Ordering::Relaxed),
			_ => {}
		}
	}

	fn strike(&self) {
		let Ok(mut recent) = self.recent.lock() else {
			return;
		};
		let now = Instant::now();
		recent.push_back(now);
		while recent.front().is_some_and(|first| now.duration_since(*first) > WINDOW) {
			recent.pop_front();
		}
		if recent.len() >= STRIKES {
			release(&format!("escape struck {STRIKES} times"));
		}
	}
}

fn release(reason: &str) -> ! {
	eprintln!("still: {reason}, releasing the keyboard");
	process::exit(0);
}
