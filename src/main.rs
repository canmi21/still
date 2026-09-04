//! Still. A keyboard you can wipe and a screen you can see the dust on.
//!
//! This binary is currently a spike: it installs the keyboard tap and nothing else, so that
//! what macOS will and will not let an application swallow is settled before any of the
//! rest is built. It has no window, no menu bar item and no configuration file yet.

mod deadline;
mod escape;
mod intercept;
mod permission;

use deadline::Deadline;
use std::time::Duration;

/// What the spike defaults to. A wedged experiment should cost a minute of a working
/// machine rather than ten, so these are far shorter than what will ship; `--idle` and
/// `--hard` override them. The settled defaults are two minutes idle and ten minutes hard,
/// and they belong in a configuration file that does not exist yet.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const HARD_TIMEOUT: Duration = Duration::from_secs(60);

fn main() {
	let (idle, hard) = timeouts();

	if !permission::granted(true) {
		eprintln!(
			"still: not trusted for Accessibility.\n\
       Grant it in System Settings > Privacy & Security > Accessibility, to whichever\n\
       application is running this binary, then start it again."
		);
		std::process::exit(1);
	}

	eprintln!(
		"still: swallowing the keyboard. Idle timeout {}s, hard timeout {}s, or strike escape\n\
     three times. Ctrl-C in another terminal, or closing this one, also releases it.",
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
fn timeouts() -> (Duration, Duration) {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let mut idle = IDLE_TIMEOUT;
	let mut hard = HARD_TIMEOUT;

	for pair in args.chunks(2) {
		let [flag, value] = pair else {
			eprintln!("still: usage: still [--idle <seconds>] [--hard <seconds>]");
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
