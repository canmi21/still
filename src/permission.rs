//! The one system permission this application cannot work without.
//!
//! A tap that filters events -- whether it drops them or hands them straight back -- requires
//! the process to be trusted for Accessibility. The grant is recorded against the code
//! signature of the running bundle rather than the path on disk, so a bare `cargo run` binary
//! is granted and revoked as its signature changes.

use objc2::rc::Retained;
use objc2_foundation::{NSDictionary, NSNumber, NSObject, NSString};
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
	fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
	/// A CFStringRef, which is the same object an NSString is.
	static kAXTrustedCheckOptionPrompt: *const NSString;
}

/// Whether this process may filter input events. Passing `prompt` opens the system dialog
/// offering to take the user to the settings pane; it appears only once per signature, so a
/// caller that has already asked gets a silent `false` on later runs.
pub fn granted(prompt: bool) -> bool {
	// SAFETY: the constant is a CFString the framework keeps alive, and Core Foundation's
	// dictionaries and Foundation's are the same objects.
	unsafe {
		let key = &*kAXTrustedCheckOptionPrompt;
		let value: Retained<NSObject> =
			Retained::into_super(Retained::into_super(NSNumber::new_bool(prompt)));
		let options: Retained<NSDictionary<NSString, NSObject>> =
			NSDictionary::from_slices(&[key], &[&*value]);
		AXIsProcessTrustedWithOptions(Retained::as_ptr(&options).cast())
	}
}
