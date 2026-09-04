//! The black screen.
//!
//! What the user is looking at while cleaning is meant to be nothing at all: on a
//! nano-texture display, a single lit pixel is enough to hide the dust beside it. So the
//! interesting question here is not how to draw a black rectangle but how little may sit on
//! top of it, and that question is easier to answer in a window that can be moved and
//! resized than in something covering every display.
//!
//! Which is why the mode exists. The two differ in how the window is configured and not at
//! all in what it contains, so what is settled here in a window carries over unchanged.

use crate::style::{Style, Watched};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
	NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
	NSColor, NSFont, NSTextAlignment, NSTextField, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSTimer};
use std::cell::RefCell;
use std::path::Path;
use std::ptr::NonNull;

/// Roughly a laptop display's proportions, so the centre stays where it will be later.
const WINDOWED_SIZE: NSSize = NSSize::new(1280.0, 800.0);

/// Tall enough for the largest hint anybody would set, and fixed, because a height that
/// changed with the font would move the line off centre every time the font did.
const HINT_HEIGHT: f64 = 48.0;

/// How often the style file is checked. Slow enough to be free, fast enough that saving the
/// file and looking up feels like one action.
const RELOAD_INTERVAL: f64 = 0.2;

/// Where the window remembers its position and size, so that a rebuild puts it back where it
/// was rather than in the middle of the screen again.
const FRAME_MEMORY: &str = "still.screen";

#[derive(Clone, Copy)]
pub enum Mode {
	/// A window with a title bar, movable and resizable. What development happens in, and the
	/// only mode where the keyboard still works.
	Windowed,
}

/// Opens the screen and runs until the application is quit. Never returns in practice.
pub fn run(mode: Mode, style_path: &Path) {
	let mtm = MainThreadMarker::new().expect("the screen must be opened from the main thread");
	let app = NSApplication::sharedApplication(mtm);
	app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

	let mut watched = Watched::new(style_path);
	let style = watched.changed().unwrap_or_default();

	let window = open(mtm, mode);
	let content = window.contentView().expect("a freshly created window has a content view");
	let hint = centred_hint(mtm, content.bounds());
	content.addSubview(&hint);
	apply(&window, &hint, &style);

	watch(window.clone(), hint.clone(), watched);

	window.makeKeyAndOrderFront(None);
	app.activate();
	app.run();
}

/// Re-reads the style file on the main run loop rather than from a thread. AppKit may only
/// be touched from the main thread, and a timer scheduled here already is one -- which is
/// simpler, and one fewer place where a mistake shows up as a crash somewhere else.
fn watch(window: Retained<NSWindow>, hint: Retained<NSTextField>, watched: Watched) {
	// The block owns both for as long as the timer lives, and the timer lives as long as the
	// run loop does. Nothing here is ever released, which is correct for a window that exists
	// for the life of the process and would be a leak in anything shorter.
	// A block must be callable from a shared reference, and re-reading the file needs a
	// unique one. The cell is safe here for the same reason the timer is: only ever the main
	// thread, only ever one call at a time.
	let watched = RefCell::new(watched);
	let tick = RcBlock::new(move |_: NonNull<NSTimer>| {
		let changed = watched.borrow_mut().changed();
		if let Some(style) = changed {
			apply(&window, &hint, &style);
		}
	});

	// SAFETY: the block only touches AppKit objects, and a timer scheduled on the main run
	// loop only ever fires on the main thread.
	unsafe {
		NSTimer::scheduledTimerWithTimeInterval_repeats_block(RELOAD_INTERVAL, true, &tick);
	}
}

fn apply(window: &NSWindow, hint: &NSTextField, style: &Style) {
	window.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(style.background, 1.0)));
	hint.setStringValue(&NSString::from_str(&style.hint));
	hint.setFont(Some(&NSFont::systemFontOfSize(style.hint_size)));
	hint.setTextColor(Some(&NSColor::colorWithWhite_alpha(style.hint_white, 1.0)));
}

fn open(mtm: MainThreadMarker, mode: Mode) -> Retained<NSWindow> {
	let (frame, style) = match mode {
		Mode::Windowed => (
			NSRect::new(NSPoint::new(0.0, 0.0), WINDOWED_SIZE),
			NSWindowStyleMask::Titled
				| NSWindowStyleMask::Closable
				| NSWindowStyleMask::Miniaturizable
				| NSWindowStyleMask::Resizable,
		),
	};

	// SAFETY: called on the main thread, with a frame and style AppKit accepts.
	let window = unsafe {
		NSWindow::initWithContentRect_styleMask_backing_defer(
			NSWindow::alloc(mtm),
			frame,
			style,
			NSBackingStoreType::Buffered,
			false,
		)
	};

	window.setTitle(&NSString::from_str("still"));
	window.setOpaque(true);
	window.center();
	window.setFrameAutosaveName(&NSString::from_str(FRAME_MEMORY));
	window
}

/// One line, centred, and staying centred as the window is resized -- a fixed height with
/// both vertical margins flexible is what pins it to the middle, and a flexible width is
/// what keeps the centred text centred.
fn centred_hint(mtm: MainThreadMarker, bounds: NSRect) -> Retained<NSTextField> {
	let hint = NSTextField::labelWithString(&NSString::from_str(""), mtm);
	hint.setAlignment(NSTextAlignment::Center);
	hint.setFrame(NSRect::new(
		NSPoint::new(0.0, (bounds.size.height - HINT_HEIGHT) / 2.0),
		NSSize::new(bounds.size.width, HINT_HEIGHT),
	));
	hint.setAutoresizingMask(
		NSAutoresizingMaskOptions::ViewWidthSizable
			| NSAutoresizingMaskOptions::ViewMinYMargin
			| NSAutoresizingMaskOptions::ViewMaxYMargin,
	);
	hint
}
