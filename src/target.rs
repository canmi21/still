//! What the tap is aiming at.
//!
//! One list, read twice. The tap builds its event mask from it; the window mode shows what
//! it would have swallowed, without swallowing anything. That they agree is the whole point
//! -- a development display that matches the tap by coincidence stops matching it the moment
//! the tap changes, and then quietly lies.
//!
//! Core Graphics and AppKit number these events identically, so the same mask serves both.

/// Key events proper. The two the gestures are read from.
pub const KEY_DOWN: u32 = 10;
pub const KEY_UP: u32 = 11;

/// Modifiers going down and coming up. Caps lock arrives here too, though dropping it does
/// not undo the state change underneath.
pub const FLAGS_CHANGED: u32 = 12;

/// Brightness, volume, the media keys and the keyboard backlight. Not key events at all,
/// which is why a mask that asks only for key events lets every one of them through.
pub const SYSTEM_DEFINED: u32 = 14;

const TARGETED: [u32; 4] = [KEY_DOWN, KEY_UP, FLAGS_CHANGED, SYSTEM_DEFINED];

/// Whether an event of this type is one the tap takes.
pub fn hits(kind: u32) -> bool {
	TARGETED.contains(&kind)
}

/// The same list as the bitmask both frameworks want it as.
pub fn mask() -> u64 {
	TARGETED.iter().fold(0, |mask, &kind| mask | (1 << kind))
}
