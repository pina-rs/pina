//! Helpers shared by the unit and integration tests.
//!
//! The library includes this file as its `test_support` module, so both test
//! layers use one implementation.

use std::io;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

/// Write `script` to `path` as an executable file without this process ever
/// opening `path`.
///
/// Linux refuses to `exec` a file that any process holds open for writing
/// (`ETXTBSY`, "Text file busy"). When a test writes a script itself, a `fork`
/// on another test thread can inherit the write descriptor and keep it until
/// that child calls `exec`, so running the script a moment later fails.
/// Renaming the file afterwards does not help, because the kernel checks the
/// file rather than its name. A short-lived shell writes the file instead, so
/// no descriptor for it ever exists in the test binary.
pub fn write_executable(path: &Path, script: &str) -> io::Result<()> {
	let status = Command::new("/bin/sh")
		.args(["-c", r#"printf '%s' "$2" > "$1" && chmod 755 "$1""#, "sh"])
		.arg(path)
		.arg(script)
		.stdin(Stdio::null())
		.status()?;

	if status.success() {
		return Ok(());
	}

	Err(io::Error::other(format!(
		"writing the executable {} failed with {status}",
		path.display()
	)))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn writes_a_runnable_script_and_reports_a_failed_write() {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("fake-command");
		write_executable(&path, "#!/bin/sh\nprintf 'ran %s' \"$1\"\n").unwrap();
		let output = Command::new(&path).arg("ok").output().unwrap();

		assert_eq!(output.stdout, b"ran ok");

		let missing = directory.path().join("missing").join("fake-command");
		let error = write_executable(&missing, "#!/bin/sh\n").unwrap_err();

		assert!(error.to_string().contains("fake-command"));
	}
}
