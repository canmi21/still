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

use crate::keys::Keystrokes;
use crate::style::{Style, Watched};
use crate::target;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
	NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
	NSColor, NSEvent, NSEventMask, NSFont, NSTextAlignment, NSTextField, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSTimer};
use std::cell::RefCell;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

/// Roughly a laptop display's proportions, so the centre stays where it will be later. Small
/// enough to sit beside an editor, since sitting beside an editor is what it is for.
const WINDOWED_SIZE: NSSize = NSSize::new(720.0, 450.0);

/// Tall enough for the largest hint anybody would set, and fixed, because a height that
/// changed with the font would move the line off centre every time the font did.
const HINT_HEIGHT: f64 = 48.0;

/// How often the style file is checked. Slow enough to be free, fast enough that saving the
/// file and looking up feels like one action.
const RELOAD_INTERVAL: f64 = 0.2;

/// The keystroke display's corner: how far it sits from the two edges it is pinned to, and
/// how tall its line is.
const KEYS_INSET: f64 = 20.0;
const KEYS_HEIGHT: f64 = 24.0;

/// Where the window remembers its position and size, so that a rebuild puts it back where it
/// was rather than in the middle of the screen again. The name is versioned because a
/// remembered frame outranks the default one: changing WINDOWED_SIZE has no visible effect
/// until the key it would be written under is a key nothing has been written to yet.
const FRAME_MEMORY: &str = "still.screen.720";

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
	let keys = cornered_keys(mtm, content.bounds());
	content.addSubview(&hint);
	content.addSubview(&keys);

	let screen = Rc::new(Screen { window, hint, keys, strokes: Keystrokes::new() });
	screen.apply(&style);
	screen.watch(watched);
	screen.listen();

	let window = screen.window.clone();

	window.makeKeyAndOrderFront(None);
	app.activate();
	app.run();
}

/// The window and everything on it, kept together because everything that changes changes
/// more than one of them at once.
struct Screen {
	window: Retained<NSWindow>,
	hint: Retained<NSTextField>,
	keys: Retained<NSTextField>,
	strokes: Keystrokes,
}

impl Screen {
	fn apply(&self, style: &Style) {
		self.window.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(style.background, 1.0)));
		self.hint.setStringValue(&NSString::from_str(&style.hint));
		self.hint.setFont(Some(&NSFont::systemFontOfSize(style.hint_size)));
		self.hint.setTextColor(Some(&NSColor::colorWithWhite_alpha(style.hint_white, 1.0)));
		self.keys.setFont(Some(&NSFont::systemFontOfSize(style.keys_size)));
		self.keys.setTextColor(Some(&NSColor::colorWithWhite_alpha(style.keys_white, 1.0)));
	}

	fn draw_keys(&self) {
		self.keys.setStringValue(&NSString::from_str(&self.strokes.line()));
	}

	/// Re-reads the style file on the main run loop rather than from a thread, and expires the
	/// keystroke display on the same tick. AppKit may only be touched from the main thread, and
	/// a timer scheduled here already is one -- which is simpler, and one fewer place where a
	/// mistake shows up as a crash somewhere else.
	fn watch(self: &Rc<Self>, watched: Watched) {
		// A block must be callable from a shared reference, and re-reading the file needs a
		// unique one. The cell is safe here for the same reason the timer is: only ever the main
		// thread, only ever one call at a time.
		let watched = RefCell::new(watched);
		let screen = Rc::clone(self);
		let linger = RefCell::new(Duration::from_secs_f64(Style::default().keys_linger));

		let tick = RcBlock::new(move |_: NonNull<NSTimer>| {
			if let Some(style) = watched.borrow_mut().changed() {
				*linger.borrow_mut() = Duration::from_secs_f64(style.keys_linger);
				screen.apply(&style);
			}
			if screen.strokes.expire(*linger.borrow()) {
				screen.draw_keys();
			}
		});

		// SAFETY: the block only touches AppKit objects, and a timer scheduled on the main run
		// loop only ever fires on the main thread.
		unsafe {
			NSTimer::scheduledTimerWithTimeInterval_repeats_block(RELOAD_INTERVAL, true, &tick);
		}
	}

	/// Watches the keys this window receives and shows the ones the tap would have taken. The
	/// event is handed straight back, which is the difference between this and the tap: the
	/// same list, read for display instead of for suppression.
	fn listen(self: &Rc<Self>) {
		let screen = Rc::clone(self);
		let seen = RcBlock::new(move |event: NonNull<NSEvent>| {
			// SAFETY: AppKit hands the monitor a live event for the duration of the call.
			let event = unsafe { event.as_ref() };
			if screen.strokes.observe(event) {
				screen.draw_keys();
			}
			(event as *const NSEvent).cast_mut()
		});

		// SAFETY: a local monitor only fires on the main thread, for this application's events.
		// The monitor is never removed, which is correct for one that lives as long as the
		// window does.
		unsafe {
			let monitor =
				NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask(target::mask()), &seen);
			std::mem::forget(monitor);
		}
	}
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

/// The keystroke display, pinned to the bottom right -- a flexible left margin and a flexible
/// top margin are what hold it there while everything around it resizes.
fn cornered_keys(mtm: MainThreadMarker, bounds: NSRect) -> Retained<NSTextField> {
	let keys = NSTextField::labelWithString(&NSString::from_str(""), mtm);
	keys.setAlignment(NSTextAlignment::Right);
	keys.setFrame(NSRect::new(
		NSPoint::new(KEYS_INSET, KEYS_INSET),
		NSSize::new(bounds.size.width - KEYS_INSET * 2.0, KEYS_HEIGHT),
	));
	keys.setAutoresizingMask(
		NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin,
	);
	keys
}
