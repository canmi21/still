//! Everything about the black screen that is worth changing without recompiling.
//!
//! The screen has almost no structure and almost nothing but appearance: how dark the dark
//! is, how visible the hint is, what the hint says. Those are exactly the questions that can
//! only be answered by looking at the thing, on the display it will be used on, so they live
//! in a file the running application re-reads rather than in constants a rebuild would carry.
//!
//! This is not only a development affordance. The settings window, when there is one, writes
//! this same file.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

pub struct Style {
	/// How light the background is, from 0 for black to 1 for white. Only ever black in use;
	/// adjustable because comparing it against a near-black is how you find out.
	pub background: f64,
	/// What the hint says, and how visible it is. Bright enough to read in a dark room, dim
	/// enough not to light the area being inspected -- the two pull against each other, and
	/// the balance is a matter of taste on a particular display.
	pub hint: String,
	pub hint_white: f64,
	pub hint_size: f64,
}

impl Default for Style {
	fn default() -> Self {
		Self {
			background: 0.0,
			hint: String::from("hold escape for three seconds to stop"),
			hint_white: 0.30,
			hint_size: 15.0,
		}
	}
}

impl Style {
	/// Reads a style file. An unknown key is an error rather than a shrug: a typo that is
	/// silently ignored looks exactly like a value that has no effect, and telling those two
	/// apart by eye is the one thing this file exists to avoid.
	pub fn parse(text: &str) -> Result<Self, String> {
		let mut style = Self::default();

		for (number, line) in text.lines().enumerate() {
			let line = line.split('#').next().unwrap_or_default().trim();
			if line.is_empty() {
				continue;
			}
			let Some((key, value)) = line.split_once('=') else {
				return Err(format!("line {}: expected `key = value`", number + 1));
			};
			let (key, value) = (key.trim(), value.trim());

			match key {
				"hint" => style.hint = value.trim_matches('"').to_owned(),
				"background" => style.background = number_at(number, value)?,
				"hint-white" => style.hint_white = number_at(number, value)?,
				"hint-size" => style.hint_size = number_at(number, value)?,
				unknown => return Err(format!("line {}: unknown key `{unknown}`", number + 1)),
			}
		}

		Ok(style)
	}
}

fn number_at(number: usize, value: &str) -> Result<f64, String> {
	value.parse().map_err(|_| format!("line {}: `{value}` is not a number", number + 1))
}

/// A style file and the modification time it was last read at, so the screen can tell a file
/// that changed from one that was merely touched by the editor's save.
pub struct Watched {
	path: Box<Path>,
	read_at: Option<SystemTime>,
}

impl Watched {
	pub fn new(path: &Path) -> Self {
		Self { path: Box::from(path), read_at: None }
	}

	/// The style, if the file has changed since the last call. A file that cannot be read or
	/// parsed reports the problem and yields nothing, which leaves the screen showing the last
	/// style that worked -- an editor that saves halfway through a line should not blank the
	/// window being looked at.
	pub fn changed(&mut self) -> Option<Style> {
		let modified = fs::metadata(&self.path).and_then(|meta| meta.modified()).ok();
		if modified.is_none() || modified == self.read_at {
			return None;
		}
		self.read_at = modified;

		match fs::read_to_string(&self.path)
			.map_err(|error| error.to_string())
			.and_then(|text| Style::parse(&text))
		{
			Ok(style) => Some(style),
			Err(problem) => {
				eprintln!("still: {}: {problem}", self.path.display());
				None
			}
		}
	}
}
