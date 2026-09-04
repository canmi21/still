//! The keyboard tap.
//!
//! It sits at the HID location, ahead of every other consumer, and is an active filter
//! rather than a passive listener. That position is the whole reason it exists: brightness,
//! volume and the media keys never reach an application's event queue at all -- the system
//! consumes them on the way -- so nothing watching from inside a window can see them, and a
//! tap below the routing sees everything.
//!
//! Filtering and dropping are not the same choice. The tap is an active filter in both
//! modes; what differs is whether the callback hands the event back. `Swallow` returns
//! nothing and the event is gone. `Observe` returns it untouched, which is how window mode
//! shows exactly what would have been taken while taking none of it.

use crate::target;
use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
	CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
	CGEventTapProxy, CGEventType,
};
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicU64, Ordering};

/// The fields holding a key event's virtual keycode, and whether the system generated it from
/// a key that is still down rather than from the user pressing one.
const KEYCODE_FIELD: CGEventField = CGEventField(9);
const AUTOREPEAT_FIELD: CGEventField = CGEventField(8);

/// Counts what was dropped, so a run can report what it actually managed to swallow.
static SWALLOWED: AtomicU64 = AtomicU64::new(0);

/// What the tap does with what it takes.
#[derive(Clone, Copy, PartialEq)]
pub enum Handling {
	/// Drop the event. Nothing downstream ever sees it.
	Swallow,
	/// Hand it back untouched, having looked at it.
	Observe,
}

/// One event, for as long as the callback is running and no longer.
pub struct Event<'tap> {
	pub kind: CGEventType,
	pub raw: &'tap CGEvent,
}

impl Event<'_> {
	pub fn keycode(&self) -> u16 {
		CGEvent::integer_value_field(Some(self.raw), KEYCODE_FIELD) as u16
	}

	pub fn is_repeat(&self) -> bool {
		CGEvent::integer_value_field(Some(self.raw), AUTOREPEAT_FIELD) != 0
	}
}

/// What the caller does with each event the tap takes.
type Seen = Box<dyn Fn(&Event)>;

struct Tapped {
	handling: Handling,
	seen: Seen,
	port: Option<CFRetained<CFMachPort>>,
}

unsafe extern "C-unwind" fn on_event(
	_proxy: CGEventTapProxy,
	kind: CGEventType,
	event: NonNull<CGEvent>,
	user_info: *mut c_void,
) -> *mut CGEvent {
	// SAFETY: the pointer was leaked by `install` and is never freed, and this callback only
	// runs on the run loop that outlives it.
	let tapped = unsafe { &*(user_info as *const Tapped) };
	let raw = unsafe { event.as_ref() };

	// The system switches a tap off if its callback is too slow, or on certain user input, and
	// says so through the callback rather than through a return value. Left alone the process
	// would sit there having quietly stopped intercepting anything.
	if matches!(kind, CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput) {
		if let Some(port) = &tapped.port {
			eprintln!("still: the system disabled the tap, re-enabling");
			CGEvent::tap_enable(port, true);
		}
		return event.as_ptr();
	}

	(tapped.seen)(&Event { kind, raw });

	match tapped.handling {
		Handling::Swallow => {
			SWALLOWED.fetch_add(1, Ordering::Relaxed);
			ptr::null_mut()
		}
		Handling::Observe => event.as_ptr(),
	}
}

/// Installs the tap on the current thread's run loop. It does not run the loop: window mode
/// hands that to AppKit, and only the bare tap runs one of its own.
pub fn install(handling: Handling, seen: Seen) -> Result<(), ()> {
	// Deliberately leaked: the callback holds a raw pointer to it for as long as the tap is
	// installed, which is until the process ends.
	let tapped = Box::into_raw(Box::new(Tapped { handling, seen, port: None }));

	// SAFETY: the callback is the one below and the pointer is the state it expects.
	let port = unsafe {
		CGEvent::tap_create(
			CGEventTapLocation::HIDEventTap,
			CGEventTapPlacement::HeadInsertEventTap,
			CGEventTapOptions::Default,
			target::mask(),
			Some(on_event),
			tapped.cast(),
		)
	}
	.ok_or(())?;

	let source = CFMachPort::new_run_loop_source(None, Some(&port), 0).ok_or(())?;
	CFRunLoop::current().ok_or(())?.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
	CGEvent::tap_enable(&port, true);

	// Handed over only now, because the callback needs the port to switch it back on and the
	// port does not exist until the tap does.
	unsafe { (*tapped).port = Some(port) };
	Ok(())
}

/// Runs the tap's own run loop. Only the bare tap uses this; window mode runs AppKit's.
pub fn run() {
	CFRunLoop::run();
}

/// How many events this run has dropped so far.
pub fn swallowed() -> u64 {
	SWALLOWED.load(Ordering::Relaxed)
}
