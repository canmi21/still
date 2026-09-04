//! The two timeouts that end a run without being asked to.
//!
//! Both are enforced from a thread that does nothing else and ends the process outright.
//! That is deliberate: the event tap dies with the process, so exiting is precisely what
//! hands the keyboard back. Nothing on the interception path can hold this off -- a wedged
//! callback, a stalled run loop and a panic all leave this thread sleeping and then calling
//! exit. It is the last guarantee, and it does not depend on any of the code above it being
//! correct.

use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How often the thread wakes to compare the clock against its two limits. Small enough
/// that the overshoot is not worth reporting, large enough to stay invisible at rest.
const TICK: Duration = Duration::from_millis(200);

pub struct Deadline {
	started: Instant,
	last_event: Arc<AtomicU64>,
}

impl Deadline {
	/// Starts the watchdog. `idle` is measured from the last intercepted event, `hard` from
	/// this call and never resets.
	pub fn start(idle: Duration, hard: Duration) -> Self {
		let started = Instant::now();
		let last_event = Arc::new(AtomicU64::new(0));
		let watched = Arc::clone(&last_event);

		thread::spawn(move || {
			loop {
				thread::sleep(TICK);
				let elapsed = started.elapsed();
				if elapsed >= hard {
					eprintln!("still: hard timeout after {}s, releasing the keyboard", hard.as_secs());
					process::exit(0);
				}
				let since_event = elapsed.as_millis() as u64 - watched.load(Ordering::Relaxed);
				if since_event >= idle.as_millis() as u64 {
					eprintln!("still: idle for {}s, releasing the keyboard", idle.as_secs());
					process::exit(0);
				}
			}
		});

		Self { started, last_event }
	}

	/// Resets the idle timeout. Called for every event the tap sees, including the ones it
	/// decides to pass through.
	pub fn touch(&self) {
		self.last_event.store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
	}
}
