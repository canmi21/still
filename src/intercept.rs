//! The keyboard tap itself.
//!
//! The tap sits at the HID location, ahead of every other consumer, and is an active filter
//! rather than a passive listener -- the two together are what let it drop an event instead
//! of merely watching it go past.
//!
//! Core Graphics is called directly here rather than through a wrapper crate. The reason is
//! the event mask: brightness, volume and the other media keys are not key events at all,
//! and the mask bit they need has no name in any Rust binding's event enum. Reaching them
//! means building the mask as the number it actually is.

use crate::deadline::Deadline;
use crate::escape::Gesture;
use core_foundation::base::TCFType;
use core_foundation::mach_port::{CFMachPort, CFMachPortRef};
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

/// Event types, as Core Graphics numbers them.
pub const KEY_DOWN: u32 = 10;
pub const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;

/// Brightness, volume, the media keys and the keyboard backlight. They travel as system
/// events rather than key events, which is why a tap that asks only for key events lets
/// every one of them straight through. Brightness is the one that matters: a cloth that
/// wakes the screen while the user is looking for dust on it defeats the whole application.
const SYSTEM_DEFINED: u32 = 14;

/// Reported to the callback rather than returned anywhere, so a tap the system has switched
/// off looks exactly like a quiet keyboard unless these are handled.
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

/// The field holding a key event's virtual keycode.
const KEYCODE_FIELD: u32 = 9;

/// Ahead of every other consumer, at the lowest point a process is allowed to sit, filtering
/// rather than watching.
const HID_LOCATION: u32 = 0;
const HEAD_INSERT: u32 = 0;
const ACTIVE_FILTER: u32 = 0;

#[repr(C)]
struct CGEventOpaque {
	_private: [u8; 0],
}
type CGEventRef = *mut CGEventOpaque;
type CGEventTapProxy = *mut c_void;
type CGEventTapCallBack =
	unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
	fn CGEventTapCreate(
		tap: u32,
		place: u32,
		options: u32,
		events_of_interest: u64,
		callback: CGEventTapCallBack,
		user_info: *mut c_void,
	) -> CFMachPortRef;
	fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
	fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
}

/// The tap's own port, so the callback can switch it back on after the system has switched
/// it off. It is a static because the callback exists before the tap does.
static PORT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Counts what was dropped, so a run can report what it actually managed to swallow.
static SWALLOWED: AtomicU64 = AtomicU64::new(0);

/// What the callback needs, reached through the tap's user pointer.
struct Tapped {
	deadline: Deadline,
	gesture: Gesture,
}

unsafe extern "C" fn on_event(
	_proxy: CGEventTapProxy,
	kind: u32,
	event: CGEventRef,
	user_info: *mut c_void,
) -> CGEventRef {
	// SAFETY: the pointer was leaked from `run` and is never freed, and this callback only
	// runs on the run loop `run` is blocked on, so it cannot outlive the allocation.
	let state = unsafe { &*(user_info as *const Tapped) };

	if kind == TAP_DISABLED_BY_TIMEOUT || kind == TAP_DISABLED_BY_USER_INPUT {
		let port = PORT.load(Ordering::Relaxed);
		if !port.is_null() {
			eprintln!("still: the system disabled the tap, re-enabling");
			unsafe { CGEventTapEnable(port.cast(), true) };
		}
		return event;
	}

	state.deadline.touch();
	SWALLOWED.fetch_add(1, Ordering::Relaxed);

	if kind == KEY_DOWN || kind == KEY_UP {
		let key = unsafe { CGEventGetIntegerValueField(event, KEYCODE_FIELD) } as u16;
		state.gesture.observe(kind, key);
	}

	ptr::null_mut()
}

/// Installs the tap and runs until something ends the process. Only returns on failure.
pub fn run(deadline: Deadline) -> Result<(), ()> {
	// Deliberately leaked: the callback holds a raw pointer to it for as long as the tap is
	// installed, which is until the process ends.
	let state = Box::into_raw(Box::new(Tapped { deadline, gesture: Gesture::watching() }));

	let mask =
		(1u64 << KEY_DOWN) | (1u64 << KEY_UP) | (1u64 << FLAGS_CHANGED) | (1u64 << SYSTEM_DEFINED);

	let port = unsafe {
		CGEventTapCreate(HID_LOCATION, HEAD_INSERT, ACTIVE_FILTER, mask, on_event, state.cast())
	};
	if port.is_null() {
		return Err(());
	}

	// SAFETY: CGEventTapCreate follows the create rule, so this takes over the one reference
	// it returned.
	let port = unsafe { CFMachPort::wrap_under_create_rule(port) };
	let source = port.create_runloop_source(0).map_err(|_| ())?;
	PORT.store(port.as_concrete_TypeRef().cast(), Ordering::Relaxed);
	CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
	unsafe { CGEventTapEnable(port.as_concrete_TypeRef(), true) };
	CFRunLoop::run_current();
	Ok(())
}

/// How many events this run has dropped so far.
pub fn swallowed() -> u64 {
	SWALLOWED.load(Ordering::Relaxed)
}
