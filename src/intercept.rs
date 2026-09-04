//! The keyboard tap itself.
//!
//! The tap sits at the HID location, ahead of every other consumer, and is an active filter
//! rather than a passive listener -- the two together are what let it drop an event instead
//! of merely watching it go past.

use crate::deadline::Deadline;
use crate::escape::Gesture;
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_graphics::event::{
	CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
	CallbackResult, EventField,
};
use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
	fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// The tap's own port, so the callback can switch it back on after the system has switched
/// it off. It is a static because the callback has to be built before the tap exists.
static PORT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Counts what went past, so a run can report what it actually managed to swallow.
static SWALLOWED: AtomicU64 = AtomicU64::new(0);

/// Installs the tap and runs until something ends the process. Only returns on failure.
pub fn run(deadline: Deadline) -> Result<(), ()> {
	let gesture = Mutex::new(Gesture::default());

	let tap = CGEventTap::new(
		CGEventTapLocation::HID,
		CGEventTapPlacement::HeadInsertEventTap,
		CGEventTapOptions::Default,
		vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
		move |_proxy, kind, event| {
			// The system switches a tap off if its callback is too slow, or on certain user
			// input, and says so through the callback rather than through a return value. Left
			// alone the process would sit there having quietly stopped intercepting anything.
			if matches!(kind, CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput) {
				let port = PORT.load(Ordering::Relaxed);
				if !port.is_null() {
					eprintln!("still: the system disabled the tap ({kind:?}), re-enabling");
					unsafe { CGEventTapEnable(port.cast(), true) };
				}
				return CallbackResult::Keep;
			}

			deadline.touch();
			SWALLOWED.fetch_add(1, Ordering::Relaxed);

			if matches!(kind, CGEventType::KeyDown) {
				let key = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
				if gesture.lock().is_ok_and(|mut g| g.observe(key)) {
					eprintln!("still: escape gesture, releasing the keyboard");
					std::process::exit(0);
				}
			}

			CallbackResult::Drop
		},
	)?;

	let source = tap.mach_port().create_runloop_source(0).map_err(|_| ())?;
	PORT.store(tap.mach_port().as_concrete_TypeRef().cast(), Ordering::Relaxed);
	CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
	tap.enable();
	CFRunLoop::run_current();
	Ok(())
}

/// How many events this run has dropped so far.
pub fn swallowed() -> u64 {
	SWALLOWED.load(Ordering::Relaxed)
}
