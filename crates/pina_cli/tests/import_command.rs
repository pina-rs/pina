//! End-to-end coverage for `pina import`: the command stamps a generated CPI
//! crate with provenance and refuses to guess when its inputs are unusable.

use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn codama_fixture() -> PathBuf {
	workspace_root().join("codama/idls/vesting_program.json")
}

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.expect("the CLI manifest lives inside the workspace")
		.to_path_buf()
}

/// The system program's well-known address: valid base58 without looking like
/// a secret.
const PROGRAM_ID: &str = "11111111111111111111111111111111";

fn run_import(arguments: &[String]) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("import")
		.args(arguments)
		.output()
		.unwrap_or_else(|error| panic!("import command failed to launch: {error}"))
}

fn output_dir(temp: &TempDir) -> String {
	temp.path().join("clients").to_string_lossy().into_owned()
}

/// The environment may force coloured output even through a pipe, so
/// assertions look at the plain text.
fn plain(bytes: &[u8]) -> String {
	let text = String::from_utf8_lossy(bytes).into_owned();
	let mut plain = String::with_capacity(text.len());
	let mut characters = text.char_indices().peekable();
	while let Some((_, character)) = characters.next() {
		if character != '\u{1b}' {
			plain.push(character);
			continue;
		}
		for (_, next) in characters.by_ref() {
			if next.is_ascii_alphabetic() {
				break;
			}
		}
	}
	plain
}

#[test]
fn imports_a_file_idl_and_stamps_provenance() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = run_import(&[
		"vesting".to_string(),
		"--program-id".to_string(),
		PROGRAM_ID.to_string(),
		"--idl".to_string(),
		codama_fixture().to_string_lossy().into_owned(),
		"--output".to_string(),
		output_dir(&temp),
	]);

	assert!(
		output.status.success(),
		"import failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = plain(&output.stdout);
	assert!(stdout.contains("Imported vesting"));
	assert!(stdout.contains("IDL sha256:"));

	let readme = std::fs::read_to_string(
		temp.path()
			.join("clients")
			.join("vesting")
			.join("README.md"),
	)
	.unwrap_or_else(|error| panic!("readme missing: {error}"));
	assert!(readme.contains("pina-import-provenance:start"));
	assert!(readme.contains(PROGRAM_ID));
}

#[test]
fn reports_an_unchanged_reimport_as_a_noop() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let arguments = [
		"vesting".to_string(),
		"--program-id".to_string(),
		PROGRAM_ID.to_string(),
		"--idl".to_string(),
		codama_fixture().to_string_lossy().into_owned(),
		"--output".to_string(),
		output_dir(&temp),
	];

	let first = run_import(&arguments);
	assert!(
		first.status.success(),
		"first import failed: {}",
		String::from_utf8_lossy(&first.stderr)
	);

	let second = run_import(&arguments);
	assert!(
		second.status.success(),
		"second import failed: {}",
		String::from_utf8_lossy(&second.stderr)
	);
	assert!(plain(&second.stdout).contains("Already up to date"));
}

#[test]
fn rejects_an_unusable_program_id() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = run_import(&[
		"vesting".to_string(),
		"--program-id".to_string(),
		"not-a-key".to_string(),
		"--idl".to_string(),
		codama_fixture().to_string_lossy().into_owned(),
		"--output".to_string(),
		output_dir(&temp),
	]);

	assert_eq!(
		output.status.code(),
		Some(1),
		"an unusable program id must exit 1"
	);
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("not a valid program address"),
		"stderr must name the bad program id"
	);
}

#[test]
fn lets_clap_reject_conflicting_idl_sources() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = run_import(&[
		"vesting".to_string(),
		"--program-id".to_string(),
		PROGRAM_ID.to_string(),
		"--idl".to_string(),
		codama_fixture().to_string_lossy().into_owned(),
		"--url".to_string(),
		"https://example.com/vesting.json".to_string(),
		"--output".to_string(),
		output_dir(&temp),
	]);

	assert_ne!(output.status.code(), Some(0));
	assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}

#[test]
fn imports_an_idl_from_a_url() {
	let body = std::fs::read(codama_fixture())
		.unwrap_or_else(|error| panic!("fixture read failed: {error}"));
	let listener =
		TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("bind failed: {error}"));
	let port = listener
		.local_addr()
		.unwrap_or_else(|error| panic!("addr failed: {error}"))
		.port();
	let server = std::thread::spawn(move || {
		let (mut stream, _) = listener
			.accept()
			.unwrap_or_else(|error| panic!("accept: {error}"));
		let request = read_request_head(&mut stream);
		assert!(request.starts_with("GET "), "expected a GET: {request}");
		let head = format!(
			"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
			 {}\r\nconnection: close\r\n\r\n",
			body.len()
		);
		stream
			.write_all(head.as_bytes())
			.unwrap_or_else(|error| panic!("head write failed: {error}"));
		stream
			.write_all(&body)
			.unwrap_or_else(|error| panic!("body write failed: {error}"));
		let _ = stream.shutdown(std::net::Shutdown::Write);
		let mut sink = [0_u8; 512];
		while matches!(stream.read(&mut sink), Ok(read) if read > 0) {}
	});

	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = run_import(&[
		"vesting".to_string(),
		"--program-id".to_string(),
		PROGRAM_ID.to_string(),
		"--url".to_string(),
		format!("http://127.0.0.1:{port}/vesting.json"),
		"--output".to_string(),
		output_dir(&temp),
	]);

	assert!(
		output.status.success(),
		"url import failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = plain(&output.stdout);
	assert!(stdout.contains("source: url"));
	assert!(
		server.join().is_ok(),
		"the one-shot server thread must not panic"
	);
}

/// Reads the request head, then any body the `content-length` header declares,
/// so the socket holds no unread bytes when the response is written. A partial
/// read makes Windows reset the connection and discard the response.
fn read_request_head(stream: &mut std::net::TcpStream) -> String {
	let mut request = Vec::new();
	let mut buffer = [0_u8; 512];
	loop {
		let read = match stream.read(&mut buffer) {
			Ok(read) => read,
			Err(error) => panic!("request read failed: {error}"),
		};
		if read == 0 {
			break;
		}
		request.extend_from_slice(&buffer[..read]);
		let text = String::from_utf8_lossy(&request);
		let Some(header_end) = text.find("\r\n\r\n") else {
			continue;
		};
		let content_length = text
			.lines()
			.find_map(|line| {
				let (name, value) = line.split_once(':')?;
				name.eq_ignore_ascii_case("content-length")
					.then(|| value.trim().to_string())
			})
			.and_then(|value| value.parse::<usize>().ok())
			.unwrap_or_default();
		if request.len() >= header_end + 4 + content_length {
			break;
		}
	}
	String::from_utf8_lossy(&request).into_owned()
}

#[test]
fn rejects_cluster_fetches_that_cannot_start() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = run_import(&[
		"vesting".to_string(),
		"--program-id".to_string(),
		"not-a-key".to_string(),
		"--cluster".to_string(),
		"localnet".to_string(),
		"--output".to_string(),
		output_dir(&temp),
	]);

	assert_eq!(
		output.status.code(),
		Some(1),
		"a cluster fetch with an unusable program id must exit 1"
	);
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("not a valid program address"),
		"stderr must name the bad program id"
	);
}
