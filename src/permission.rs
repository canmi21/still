//! The one system permission this application cannot work without.
//!
//! Suppressing keyboard events requires the process to be trusted for Accessibility. The
//! grant is recorded against the code signature of the running bundle, not against the path
//! on disk, so a bare `cargo run` binary is granted and revoked as its signature changes.

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
	fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
	static kAXTrustedCheckOptionPrompt: CFStringRef;
}

/// Whether this process may suppress input events. Passing `prompt` opens the system dialog
/// that offers to take the user to the settings pane; it appears only once per signature,
/// so a caller that has already asked gets a silent `false` on later runs.
pub fn granted(prompt: bool) -> bool {
	let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
	let options =
		CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::from(prompt).as_CFType())]);
	unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
}
