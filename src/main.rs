//! Still. A keyboard you can wipe and a screen you can see the dust on.
//!
//! Two spikes so far, and they are deliberately separate commands. `tap` swallows the
//! keyboard and has no window; `screen` opens the black screen and leaves the keyboard
//! alone. Keeping them apart is what makes the screen safe to iterate on -- a window whose
//! layout is being fiddled with should not also be holding the keyboard hostage.

mod deadline;
mod escape;
mod intercept;
mod keys;
mod permission;
mod screen;
mod style;
mod target;

use deadline::Deadline;
use screen::Mode;
use std::path::Path;
use std::time::Duration;

/// What the spike defaults to. A wedged experiment should cost a minute of a working machine
/// rather than ten, so these are far shorter than what will ship; `--idle` and `--hard`
/// override them. The settled defaults are two minutes idle and ten minutes hard, and they
/// belong in a configuration file that does not exist yet.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const HARD_TIMEOUT: Duration = Duration::from_secs(60);

/// Re-read while the screen is open, so appearance can be settled by looking at it.
const STYLE: &str = "screen.style";

const USAGE: &str = "still: usage: still tap [--idle <seconds>] [--hard <seconds>]\n\
                     still: usage: still screen";

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	match args.split_first() {
		Some((command, rest)) if command == "tap" => tap(rest),
		Some((command, [])) if command == "screen" => screen::run(Mode::Windowed, Path::new(STYLE)),
		_ => {
			eprintln!("{USAGE}");
			std::process::exit(2);
		}
	}
}

fn tap(args: &[String]) {
	let (idle, hard) = timeouts(args);

	if !permission::granted(true) {
		eprintln!(
			"still: not trusted for Accessibility.\n\
       Grant it in System Settings > Privacy & Security > Accessibility, to whichever\n\
       application is running this binary, then start it again."
		);
		std::process::exit(1);
	}

	eprintln!(
		"still: swallowing the keyboard. Idle timeout {}s, hard timeout {}s, or hold escape\n\
     for three seconds. Ctrl-C in another terminal, or closing this one, also releases it.",
		idle.as_secs(),
		hard.as_secs()
	);

	let deadline = Deadline::start(idle, hard);
	if intercept::run(deadline).is_err() {
		eprintln!("still: could not install the event tap");
		std::process::exit(1);
	}
	eprintln!("still: released after swallowing {} events", intercept::swallowed());
}

/// `--idle <seconds>` and `--hard <seconds>`, and nothing else. A background daemon that
/// wants to stay small does not need an argument parser to read two numbers.
fn timeouts(args: &[String]) -> (Duration, Duration) {
	let mut idle = IDLE_TIMEOUT;
	let mut hard = HARD_TIMEOUT;

	for pair in args.chunks(2) {
		let [flag, value] = pair else {
			eprintln!("{USAGE}");
			std::process::exit(2);
		};
		let Ok(seconds) = value.parse::<u64>() else {
			eprintln!("still: {value} is not a number of seconds");
			std::process::exit(2);
		};
		match flag.as_str() {
			"--idle" => idle = Duration::from_secs(seconds),
			"--hard" => hard = Duration::from_secs(seconds),
			other => {
				eprintln!("still: unknown option {other}");
				std::process::exit(2);
			}
		}
	}

	(idle, hard)
}
