//! End-to-end test: drives the generated CLI binary against a live
//! `@solana/surfpool` surfnet, one call per CLI command. Each invocation
//! below is spelled out exactly as a user would type it, so this file also
//! documents how the generated CLI is meant to be used.
//!
//! Requires node with `@solana/surfpool` installed (run `pnpm install` at the
//! repository root) and the counter SBF artifact at
//! `target/surfpool/examples/counter_program.so`:
//!
//! ```sh
//! cargo build-sbf --features bpf-entrypoint \
//!   --manifest-path examples/counter_program/Cargo.toml \
//!   --sbf-out-dir target/surfpool/examples
//! cargo test -p counter_program_cli --test surfpool -- --ignored
//! ```

use std::io::BufRead;
use std::process::Command;
use std::process::Stdio;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;

const COUNTER_PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
const PROGRAM_SO: &str = "target/surfpool/examples/counter_program.so";

#[ignore = "requires node + @solana/surfpool and a built counter_program SBF artifact"]
#[test]
fn cli_initializes_increments_and_fetches_against_surfpool()
-> Result<(), Box<dyn std::error::Error>> {
	// A fresh payer keypair, funded by the bootstrap once the surfnet is up.
	let payer = Keypair::new();
	let keypair_path = write_keypair_file(&payer)?;

	// Boot a surfnet with the counter program deployed at its declared ID.
	let mut bootstrap = spawn_surfnet(&payer.pubkey())?;
	let rpc_url = wait_for_ready(&mut bootstrap)?;

	// Canonical bump for the counter PDA, derived from the payer's authority.
	let (_, bump) = counter_pda(payer.pubkey());

	// counter-program-cli initialize --url <surfnet> --keypair <payer> \
	//     [--bump 254]                       # counter PDA is derived automatically
	cli(&rpc_url, &keypair_path)?
		.arg("initialize")
		.arg("--bump")
		.arg(bump.to_string())
		.run()?;

	// counter-program-cli increment --url <surfnet> --keypair <payer>
	cli(&rpc_url, &keypair_path)?.arg("increment").run()?;

	// counter-program-cli increment --simulate --url <surfnet> --keypair <payer>
	//
	// Every command requests the compute unit limit recorded for its
	// instruction, and a simulation reports what it consumed against it.
	let limit = counter_program_client::instructions::INCREMENT_COMPUTE_UNIT_LIMIT;
	let simulated = cli(&rpc_url, &keypair_path)?
		.args(&["increment", "--simulate"])
		.run()?;
	assert!(
		simulated.contains(&format!(" of {limit} requested compute units")),
		"simulate must report consumption against the recorded limit, got: {simulated}"
	);

	// counter-program-cli increment --simulate --compute-unit-limit 200 ...
	//
	// The flag replaces the recorded limit, so a budget below what the
	// instruction needs exhausts the meter.
	let starved = cli(&rpc_url, &keypair_path)?
		.args(&["increment", "--simulate", "--compute-unit-limit", "200"])
		.run()
		.expect_err("a 200 unit budget cannot cover increment");
	assert!(
		starved.to_string().contains("exceeded CUs meter"),
		"the override must reach the transaction, got: {starved}"
	);

	// counter-program-cli fetch counter-state --url <surfnet> --keypair <payer> \
	//     --authority <payer pubkey> --json
	let output = cli(&rpc_url, &keypair_path)?
		.args(&[
			"fetch",
			"counter-state",
			"--authority",
			&payer.pubkey().to_string(),
			"--json",
		])
		.run()?;

	assert!(
		output.contains("\"count\": 1"),
		"fetch must report one increment after initialize + increment, got: {output}"
	);
	// The counter stores no authority field: `--authority` seeds the PDA the
	// fetch derives, so finding the account already proves whose it is.
	assert!(
		output.contains(&format!("\"bump\": {bump}")),
		"fetch must report the canonical bump {bump}, got: {output}"
	);

	Ok(())
}

/// The bootstrap process. It is killed when the test ends, pass or fail, so a
/// failed assertion or early return cannot leak a running surfnet.
struct Bootstrap(std::process::Child);

impl Drop for Bootstrap {
	fn drop(&mut self) {
		let _ = self.0.kill();
		let _ = self.0.wait();
	}
}

/// A pending CLI invocation: global flags first, subcommand + args after.
struct Cli {
	command: Command,
}

impl Cli {
	fn arg(mut self, value: impl Into<String>) -> Self {
		self.command.arg(value.into());
		self
	}

	fn args(mut self, values: &[&str]) -> Self {
		self.command.args(values);
		self
	}

	fn run(mut self) -> Result<String, Box<dyn std::error::Error>> {
		let output = self.command.output()?;
		let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
		let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
		if !output.status.success() {
			return Err(format!("CLI failed ({stdout}{stderr})").into());
		}
		Ok(stdout)
	}
}

/// Global flags shared by every invocation: the surfnet RPC URL, the payer
/// keypair, and the deployed program address.
fn cli(rpc_url: &str, keypair_path: &str) -> Result<Cli, Box<dyn std::error::Error>> {
	let mut command = Command::new(env!("CARGO_BIN_EXE_counter-program-cli"));
	command
		.args(["--url", rpc_url])
		.args(["--keypair", keypair_path])
		.args(["--program-id", COUNTER_PROGRAM_ID]);
	Ok(Cli { command })
}

fn spawn_surfnet(payer: &Pubkey) -> Result<Bootstrap, Box<dyn std::error::Error>> {
	let so_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.find(|ancestor| ancestor.join(".git").exists())
		.expect("crate lives inside the Pina repository checkout")
		.join(PROGRAM_SO);

	let child = Command::new("node")
		.arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/surfpool.mjs"))
		.arg(&so_path)
		.arg(COUNTER_PROGRAM_ID)
		.arg(payer.to_string())
		.current_dir(std::env::temp_dir())
		.stdout(Stdio::piped())
		// A bootstrap failure, such as an RPC that never answers, explains
		// itself on stderr.
		.stderr(Stdio::inherit())
		.spawn()?;
	Ok(Bootstrap(child))
}

fn counter_pda(authority: Pubkey) -> (Pubkey, u8) {
	let program = Pubkey::from_str_const(COUNTER_PROGRAM_ID);
	Pubkey::find_program_address(&[b"counter", authority.as_ref()], &program)
}

fn write_keypair_file(payer: &Keypair) -> Result<String, Box<dyn std::error::Error>> {
	let path = std::env::temp_dir().join(format!("pina-cli-e2e-{}.json", payer.pubkey()));
	let bytes = payer.to_bytes();
	let json = format!(
		"[{}]",
		bytes
			.iter()
			.map(|byte| byte.to_string())
			.collect::<Vec<_>>()
			.join(",")
	);
	std::fs::write(&path, json)?;
	Ok(path.to_string_lossy().into_owned())
}

/// Reads the `READY <url>` line the bootstrap script prints once the program
/// is deployed, the payer funded, and the RPC has answered a request. Lines
/// are consumed incrementally so a URL is acted on the moment it appears
/// instead of after the process exits.
fn wait_for_ready(bootstrap: &mut Bootstrap) -> Result<String, Box<dyn std::error::Error>> {
	let mut reader = std::io::BufReader::new(bootstrap.0.stdout.as_mut().expect("piped stdout"));
	let mut line = String::new();
	loop {
		line.clear();
		let read = reader.read_line(&mut line)?;
		if read == 0 {
			return Err("bootstrap did not report a URL before exiting".into());
		}
		if let Some(url) = line.strip_prefix("READY ") {
			return Ok(url.trim().to_string());
		}
	}
}
