//! What the window mode shows instead of swallowing.
//!
//! A keystroke display, in the corner, listing the keys that hit the interception list. It
//! is honest by construction rather than by care: it asks `target` the same question the tap
//! asks, so a key it shows is a key the tap would have taken, and a key it stays silent
//! about is one the tap would have let past.

use crate::intercept::Event;
use crate::target;
use objc2_app_kit::{NSEvent, NSEventModifierFlags};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How many keys stay on screen at once. Enough to see a chord and what followed it.
const SHOWN: usize = 8;

/// Keys whose names are not the character they type. Everything absent from here is spelled
/// by whatever the key would have typed with its modifiers ignored.
const NAMED: [(u16, &str); 24] = [
	(0x24, "return"),
	(0x30, "tab"),
	(0x31, "space"),
	(0x33, "delete"),
	(0x35, "esc"),
	(0x39, "caps"),
	(0x60, "F5"),
	(0x61, "F6"),
	(0x62, "F7"),
	(0x63, "F3"),
	(0x64, "F8"),
	(0x65, "F9"),
	(0x67, "F11"),
	(0x6D, "F10"),
	(0x6F, "F12"),
	(0x73, "home"),
	(0x74, "page up"),
	(0x75, "fwd delete"),
	(0x76, "F4"),
	(0x77, "end"),
	(0x78, "F2"),
	(0x79, "page down"),
	(0x7A, "F1"),
	(0x7B, "left"),
];

const ARROWS: [(u16, &str); 3] = [(0x7C, "right"), (0x7D, "down"), (0x7E, "up")];

pub struct Keystrokes {
	recent: RefCell<VecDeque<(Instant, String)>>,
}

impl Keystrokes {
	pub fn new() -> Self {
		Self { recent: RefCell::new(VecDeque::new()) }
	}

	/// Records one event, if it is one the tap would have taken. Returns whether the display
	/// changed, so the caller can avoid redrawing a line that has not moved.
	pub fn observe(&self, event: &Event) -> bool {
		if !target::hits(event.kind.0) {
			return false;
		}
		// AppKit's reading of the same event. A media key's identity lives in fields only
		// NSEvent spells out, and the bridge costs nothing, so both kinds go through it.
		let Some(event) = NSEvent::eventWithCGEvent(event.raw) else {
			return false;
		};

		// Only the way down. A key-up for every key-down would double the line and say nothing
		// the key-down did not already say.
		let name = match event.r#type().0 as u32 {
			target::KEY_DOWN => describe(&event),
			target::SYSTEM_DEFINED => describe_media(&event),
			_ => None,
		};
		let Some(name) = name else {
			return false;
		};

		let mut recent = self.recent.borrow_mut();
		recent.push_back((Instant::now(), name));
		while recent.len() > SHOWN {
			recent.pop_front();
		}
		true
	}

	/// Drops what has been on screen longer than `linger`. Returns whether anything went.
	pub fn expire(&self, linger: Duration) -> bool {
		let mut recent = self.recent.borrow_mut();
		let now = Instant::now();
		let before = recent.len();
		while recent.front().is_some_and(|(at, _)| now.duration_since(*at) > linger) {
			recent.pop_front();
		}
		recent.len() != before
	}

	pub fn line(&self) -> String {
		self.recent.borrow().iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>().join("   ")
	}
}

/// The keys that never reach an application: brightness, volume, playback and the keyboard
/// backlight. Their identity is packed into one integer rather than given a keycode, which is
/// the surface sign of their travelling a different road entirely.
const MEDIA: [(isize, &str); 13] = [
	(0, "volume up"),
	(1, "volume down"),
	(2, "brightness up"),
	(3, "brightness down"),
	(7, "mute"),
	(16, "play"),
	(17, "next"),
	(18, "previous"),
	(19, "fast forward"),
	(20, "rewind"),
	(21, "backlight up"),
	(22, "backlight down"),
	(23, "backlight toggle"),
];

/// The subtype that marks a system event as one of the keys above rather than as one of the
/// window server's own errands.
const AUX_CONTROL_BUTTONS: i16 = 8;

fn describe_media(event: &NSEvent) -> Option<String> {
	if event.subtype().0 != AUX_CONTROL_BUTTONS {
		return None;
	}
	let data = event.data1();
	// Down only, for the same reason key-ups are skipped.
	if (data & 0xFF00) >> 8 != 0x0A {
		return None;
	}
	let key = (data & 0xFFFF_0000) >> 16;
	let (_, name) = MEDIA.iter().find(|(code, _)| *code == key)?;
	Some((*name).to_owned())
}

/// One key, spelled the way the keyboard is labelled: modifiers first, then the key itself.
fn describe(event: &NSEvent) -> Option<String> {
	let mut name = String::new();
	let flags = event.modifierFlags();
	for (flag, symbol) in [
		(NSEventModifierFlags::Control, "control"),
		(NSEventModifierFlags::Option, "option"),
		(NSEventModifierFlags::Shift, "shift"),
		(NSEventModifierFlags::Command, "command"),
	] {
		if flags.contains(flag) {
			name.push_str(symbol);
			name.push('-');
		}
	}

	let code = event.keyCode();
	if let Some((_, named)) = NAMED.iter().chain(ARROWS.iter()).find(|(key, _)| *key == code) {
		name.push_str(named);
	} else {
		let typed = event.charactersIgnoringModifiers()?.to_string();
		if typed.trim().is_empty() {
			name.push_str(&format!("key {code}"));
		} else {
			name.push_str(&typed.to_uppercase());
		}
	}

	if event.isARepeat() {
		name.push_str(" (held)");
	}
	Some(name)
}
