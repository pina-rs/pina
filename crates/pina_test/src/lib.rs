//! Focused, host-side Surfpool integration test support for Pina programs.
//!
//! [`OfflineSurfnet`] always starts with upstream RPC access disabled, owns its
//! dynamically allocated ports, and requests synchronous shutdown. Its inner
//! Surfnet also requests shutdown from `Drop`, including during a panic.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

pub use solana_account::Account;
pub use solana_instruction::AccountMeta;
pub use solana_instruction::Instruction;
pub use solana_keypair::Keypair;
use solana_message::Message;
pub use solana_pubkey::Pubkey;
pub use solana_rent::Rent;
pub use solana_signature::Signature;
pub use solana_signer::Signer;
use solana_transaction::Transaction;
use surfpool_sdk::Surfnet;
use surfpool_sdk::cheatcodes::builders::DeployProgram;
use surfpool_sdk::cheatcodes::builders::SetAccount;

static BENCHMARK_RECORD_LOCK: Mutex<()> = Mutex::new(());
const TEST_PAYER_SEED: [u8; 32] = [0xA5; 32];

/// Run an async integration-test body on a dedicated Tokio runtime.
///
/// Test bodies panic through this helper, so panics are intercepted while the
/// Surfpool runtime drains and then re-raised after shutdown completes. This
/// keeps failing tests from aborting the process while their instance tears
/// down in the background.
///
/// # Panics
///
/// Panics if Tokio cannot construct the test runtime, and re-panics with the
/// original payload when the test body panics.
pub fn run<F>(future: F) -> F::Output
where
	F: Future,
{
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.unwrap_or_else(|error| panic!("build Pina integration-test runtime: {error}"));

	match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.block_on(future))) {
		Ok(output) => output,
		Err(payload) => std::panic::resume_unwind(payload),
	}
}

/// Return whether the suite was started with `pina test --compatibility`.
///
/// Compatibility mode still runs the complete Surfpool suite. Tests can use
/// this signal to add expensive historical matrices while preserving all
/// current-flow assertions in the same run.
#[must_use]
pub fn compatibility_mode() -> bool {
	std::env::var_os("PINA_COMPATIBILITY").is_some_and(|value| value == "1")
}

/// An error from an isolated Pina integration-test operation.
#[derive(Debug, thiserror::Error)]
#[error("{operation}: {message}")]
pub struct TestError {
	operation: &'static str,
	message: String,
}

impl TestError {
	/// Short name of the operation that failed.
	#[must_use]
	pub const fn operation(&self) -> &'static str {
		self.operation
	}

	/// Error text returned by the underlying runtime or RPC client.
	#[must_use]
	pub fn message(&self) -> &str {
		&self.message
	}
}

/// Exact bytes and account metas captured from one historical client version.
///
/// `version` is diagnostic metadata. Pina never rewrites `data` or fills in
/// account metas on the host: the deployed program must accept the old request
/// exactly as a released client produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalInstruction {
	version: u32,
	data: Vec<u8>,
	accounts: Vec<AccountMeta>,
}

impl HistoricalInstruction {
	/// Create an immutable historical request from golden wire bytes.
	#[must_use]
	pub fn new(version: u32, data: impl Into<Vec<u8>>, accounts: Vec<AccountMeta>) -> Self {
		Self {
			version,
			data: data.into(),
			accounts,
		}
	}

	/// Historical migration version named by the fixture.
	#[must_use]
	pub const fn version(&self) -> u32 {
		self.version
	}

	/// Exact historical instruction bytes.
	#[must_use]
	pub fn data(&self) -> &[u8] {
		&self.data
	}

	/// Exact positional account list and privileges sent by the old client.
	#[must_use]
	pub fn accounts(&self) -> &[AccountMeta] {
		&self.accounts
	}

	fn instruction(&self, program_id: Pubkey) -> Instruction {
		Instruction::new_with_bytes(program_id, &self.data, self.accounts.clone())
	}
}

/// Exact event bytes captured from one released program version.
///
/// Keep these bytes as a golden fixture and pass them to the event type's
/// generated projection API. Encoding the fixture with the current event type
/// would only test the current schema and can conceal a broken decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalEvent {
	version: u32,
	data: Vec<u8>,
}

impl HistoricalEvent {
	/// Create one immutable event fixture from released wire bytes.
	#[must_use]
	pub fn new(version: u32, data: impl Into<Vec<u8>>) -> Self {
		Self {
			version,
			data: data.into(),
		}
	}

	/// Historical migration version named by the fixture.
	#[must_use]
	pub const fn version(&self) -> u32 {
		self.version
	}

	/// Exact discriminator, version, and payload bytes emitted historically.
	#[must_use]
	pub fn data(&self) -> &[u8] {
		&self.data
	}
}

/// Exact account state captured for one historical on-chain schema version.
///
/// The data includes the discriminator and migration version envelope. Tests
/// install these bytes directly instead of constructing them with the current
/// generated account type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalAccount {
	version: u32,
	address: Pubkey,
	account: Account,
}

impl HistoricalAccount {
	/// Create a rent-exempt, non-executable historical account fixture.
	#[must_use]
	pub fn new(version: u32, address: Pubkey, owner: Pubkey, data: impl Into<Vec<u8>>) -> Self {
		let data = data.into();
		let lamports = Rent::default().minimum_balance(data.len());

		Self {
			version,
			address,
			account: Account {
				lamports,
				data,
				owner,
				executable: false,
				rent_epoch: 0,
			},
		}
	}

	/// Override lamports to exercise rent deficits, surplus retention, or overflow.
	#[must_use]
	pub const fn with_lamports(mut self, lamports: u64) -> Self {
		self.account.lamports = lamports;
		self
	}

	/// Historical migration version named by the fixture.
	#[must_use]
	pub const fn version(&self) -> u32 {
		self.version
	}

	/// Address where this state will be installed.
	#[must_use]
	pub const fn address(&self) -> Pubkey {
		self.address
	}

	/// Complete account state that will be installed.
	#[must_use]
	pub const fn account(&self) -> &Account {
		&self.account
	}
}

/// Exact pre-transaction state used to prove Solana rollback behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSnapshot {
	address: Pubkey,
	account: Account,
}

impl AccountSnapshot {
	/// Snapshotted account address.
	#[must_use]
	pub const fn address(&self) -> Pubkey {
		self.address
	}

	/// Complete snapshotted account state.
	#[must_use]
	pub const fn account(&self) -> &Account {
		&self.account
	}
}

/// A deployed Pina program running in its own isolated Surfpool instance.
///
/// `pina test` sets `PINA_SBF_ARTIFACT` before it runs the dedicated Surfpool
/// test package. The performance harness can instead supply a program manifest.
/// [`ProgramTest::start`] resolves the artifact, deploys the program, and leaves
/// tests to focus on instructions and state assertions.
pub struct ProgramTest {
	benchmark_program: Option<String>,
	program_id: Pubkey,
	surfnet: OfflineSurfnet,
}

impl ProgramTest {
	/// Start an offline Surfnet and deploy the artifact supplied by `pina test`.
	///
	/// # Errors
	///
	/// Returns an error when no supplied artifact names a file, or the Surfnet
	/// cannot start and deploy the program.
	pub async fn start(program_id: Pubkey) -> Result<Self, TestError> {
		let (artifact, benchmark_program) = artifact_from_env(&program_id)?;
		let mut test = Self::start_with_artifact(program_id, &artifact).await?;
		test.benchmark_program = benchmark_program;

		Ok(test)
	}

	/// Start an offline Surfnet and deploy an explicit SBF artifact.
	///
	/// # Errors
	///
	/// Returns an error when `artifact` does not name a file or the Surfnet cannot
	/// start and deploy the program.
	pub async fn start_with_artifact(
		program_id: Pubkey,
		artifact: &Path,
	) -> Result<Self, TestError> {
		if !artifact.is_file() {
			return Err(test_error(
				"locate SBF program artifact",
				format_args!("missing file: {}", artifact.display()),
			));
		}

		let surfnet = OfflineSurfnet::start().await?;
		surfnet.deploy_program(program_id, artifact)?;

		Ok(Self {
			benchmark_program: None,
			program_id,
			surfnet,
		})
	}

	/// Address where the program is deployed.
	#[must_use]
	pub const fn program_id(&self) -> Pubkey {
		self.program_id
	}

	/// Address of the pre-funded transaction payer.
	#[must_use]
	pub fn payer(&self) -> Pubkey {
		self.surfnet.payer()
	}

	/// Build an instruction addressed to the deployed program.
	#[must_use]
	pub fn instruction(&self, data: &[u8], accounts: Vec<AccountMeta>) -> Instruction {
		Instruction::new_with_bytes(self.program_id, data, accounts)
	}

	/// Build, sign with the pre-funded payer, submit, and confirm one program instruction.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, submission, or confirmation fails.
	pub fn send(&self, data: &[u8], accounts: Vec<AccountMeta>) -> Result<Signature, TestError> {
		self.send_instruction(self.instruction(data, accounts))
	}

	/// Sign with the pre-funded payer, submit, and confirm one instruction.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, submission, or confirmation fails.
	pub fn send_instruction(&self, instruction: Instruction) -> Result<Signature, TestError> {
		self.surfnet.send_program_instruction(
			self.program_id,
			self.benchmark_program.as_deref(),
			instruction,
		)
	}

	/// Submit and confirm one instruction with the payer and additional signers.
	///
	/// The payer is always the transaction fee payer and first signer. Callers
	/// only provide program-specific signers.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, signing, submission, or
	/// confirmation fails.
	pub fn send_with_signers(
		&self,
		instruction: Instruction,
		signers: &[&dyn Signer],
	) -> Result<Signature, TestError> {
		self.surfnet.send_program_instruction_with_signers(
			self.program_id,
			self.benchmark_program.as_deref(),
			instruction,
			signers,
		)
	}

	/// Install exact historical account bytes owned by this program.
	///
	/// # Errors
	///
	/// Returns an error when Surfpool rejects the direct fixture update.
	pub fn install_historical_account(&self, fixture: &HistoricalAccount) -> Result<(), TestError> {
		if fixture.account.owner != self.program_id {
			return Err(test_error(
				"install historical account",
				format_args!(
					"fixture owner {} does not match deployed program {}",
					fixture.account.owner, self.program_id
				),
			));
		}

		self.surfnet.install_historical_account(fixture)
	}

	/// Submit one exact historical request to the latest deployed program.
	///
	/// # Errors
	///
	/// Returns an error when signing, submission, execution, or confirmation fails.
	pub fn send_historical_instruction(
		&self,
		fixture: &HistoricalInstruction,
		signers: &[&dyn Signer],
	) -> Result<Signature, TestError> {
		self.surfnet
			.send_historical_instruction(self.program_id, fixture, signers)
	}

	/// Capture complete state for accounts that a rejected migration may touch.
	///
	/// # Errors
	///
	/// Returns an error when any account cannot be fetched.
	pub fn snapshot_accounts(
		&self,
		addresses: &[Pubkey],
	) -> Result<Vec<AccountSnapshot>, TestError> {
		self.surfnet.snapshot_accounts(addresses)
	}

	/// Require a historical request to fail in program execution and prove that
	/// every protected account rolled back byte-for-byte.
	///
	/// Errors before program execution, such as missing transaction signatures or
	/// RPC failure, do not satisfy the assertion.
	///
	/// # Errors
	///
	/// Returns an error when the request succeeds, fails before execution, or any
	/// protected account differs after rejection.
	pub fn expect_historical_rejection_with_rollback(
		&self,
		fixture: &HistoricalInstruction,
		protected_accounts: &[Pubkey],
		signers: &[&dyn Signer],
	) -> Result<(), TestError> {
		self.surfnet.expect_historical_rejection_with_rollback(
			self.program_id,
			fixture,
			protected_accounts,
			signers,
		)
	}

	/// Fund an address inside the isolated Surfnet.
	///
	/// # Errors
	///
	/// Returns an error when the local RPC rejects or cannot confirm the airdrop.
	pub fn fund(&self, address: &Pubkey, lamports: u64) -> Result<Signature, TestError> {
		self.surfnet.fund(address, lamports)
	}

	/// Fetch an account from the isolated Surfnet.
	///
	/// # Errors
	///
	/// Returns an error when the account does not exist or the local RPC fails.
	pub fn account(&self, address: &Pubkey) -> Result<Account, TestError> {
		self.surfnet.account(address)
	}

	/// Fetch an address balance from the isolated Surfnet.
	///
	/// # Errors
	///
	/// Returns an error when the local RPC fails.
	pub fn balance(&self, address: &Pubkey) -> Result<u64, TestError> {
		self.surfnet.balance(address)
	}

	/// Return whether the deployed program account exists and is executable.
	///
	/// # Errors
	///
	/// Returns an error when the program account cannot be fetched.
	pub fn is_executable(&self) -> Result<bool, TestError> {
		self.surfnet.program_is_executable(&self.program_id)
	}

	/// Stop the owned Surfpool RPC servers and release their ports.
	///
	/// # Errors
	///
	/// Returns an error when both RPC servers do not confirm shutdown in time.
	pub fn stop(&mut self) -> Result<(), TestError> {
		self.surfnet.stop()
	}
}

/// A dynamically ported Surfpool instance with upstream network access off.
pub struct OfflineSurfnet {
	inner: Surfnet,
}

impl Drop for OfflineSurfnet {
	fn drop(&mut self) {
		// Dropping `Surfnet` hands a Terminate command to a runtime that may
		// already be gone, which aborts the whole test process when a test
		// panics. Drain synchronously here before the unwinder continues.
		let _ = self.inner.stop();
	}
}

impl OfflineSurfnet {
	/// Build a system-program transfer instruction from the payer.
	fn transfer_instruction(&self, to: &Pubkey, lamports: u64) -> Instruction {
		let mut data = Vec::with_capacity(12);
		// System program `Transfer` enum tag = 2.
		data.extend_from_slice(&2u32.to_le_bytes());
		data.extend_from_slice(&lamports.to_le_bytes());

		Instruction::new_with_bytes(
			system_program_id(),
			&data,
			vec![
				AccountMeta::new(self.payer(), true),
				AccountMeta::new(*to, false),
			],
		)
	}

	/// Start an isolated Surfpool instance without contacting an upstream RPC.
	///
	/// # Errors
	///
	/// Returns an error when Surfpool cannot allocate ports or start its runtime.
	pub async fn start() -> Result<Self, TestError> {
		let inner = Surfnet::builder()
			.offline(true)
			.payer(Keypair::new_from_array(TEST_PAYER_SEED))
			.start()
			.await
			.map_err(|error| test_error("start offline Surfpool", error))?;

		Ok(Self { inner })
	}

	/// Address of the pre-funded transaction payer.
	#[must_use]
	pub fn payer(&self) -> Pubkey {
		self.inner.payer().pubkey()
	}

	/// Deploy an SBF artifact directly at its declared program address.
	///
	/// # Errors
	///
	/// Returns an error when the artifact cannot be read or deployed.
	pub fn deploy_program(&self, program_id: Pubkey, artifact: &Path) -> Result<(), TestError> {
		self.inner
			.cheatcodes()
			.deploy(DeployProgram::new(program_id).so_path(artifact))
			.map(|_| ())
			.map_err(|error| test_error("deploy SBF program", error))
	}

	/// Install one exact historical account fixture without running a transaction.
	///
	/// # Errors
	///
	/// Returns an error when Surfpool rejects the direct state update.
	pub fn install_historical_account(&self, fixture: &HistoricalAccount) -> Result<(), TestError> {
		self.inner
			.cheatcodes()
			.execute(
				SetAccount::new(fixture.address)
					.lamports(fixture.account.lamports)
					.owner(fixture.account.owner)
					.data(fixture.account.data.clone())
					.rent_epoch(fixture.account.rent_epoch)
					.executable(fixture.account.executable),
			)
			.map_err(|error| test_error("install historical account", error))
	}

	/// Return whether the deployed account exists and is executable.
	///
	/// # Errors
	///
	/// Returns an error when the account cannot be fetched from the local RPC.
	pub fn program_is_executable(&self, program_id: &Pubkey) -> Result<bool, TestError> {
		self.inner
			.rpc_client()
			.get_account(program_id)
			.map(|account| account.executable)
			.map_err(|error| test_error("fetch deployed program account", error))
	}

	/// Sign, submit, and confirm one instruction with the pre-funded payer.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, submission, or confirmation fails.
	pub fn send_instruction(&self, instruction: Instruction) -> Result<Signature, TestError> {
		self.send_instruction_with_signers_inner(instruction, &[], None)
	}

	/// Sign, submit, and confirm one instruction with additional signers.
	///
	/// The pre-funded payer remains the fee payer and is added automatically.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, signing, submission, or
	/// confirmation fails.
	pub fn send_instruction_with_signers(
		&self,
		instruction: Instruction,
		signers: &[&dyn Signer],
	) -> Result<Signature, TestError> {
		self.send_instruction_with_signers_inner(instruction, signers, None)
	}

	fn send_program_instruction(
		&self,
		program_id: Pubkey,
		benchmark_program: Option<&str>,
		instruction: Instruction,
	) -> Result<Signature, TestError> {
		let record = (instruction.program_id == program_id)
			.then_some(benchmark_program)
			.flatten();

		self.send_instruction_with_signers_inner(instruction, &[], record)
	}

	fn send_program_instruction_with_signers(
		&self,
		program_id: Pubkey,
		benchmark_program: Option<&str>,
		instruction: Instruction,
		signers: &[&dyn Signer],
	) -> Result<Signature, TestError> {
		let record = (instruction.program_id == program_id)
			.then_some(benchmark_program)
			.flatten();

		self.send_instruction_with_signers_inner(instruction, signers, record)
	}

	fn send_instruction_with_signers_inner(
		&self,
		instruction: Instruction,
		signers: &[&dyn Signer],
		record_program: Option<&str>,
	) -> Result<Signature, TestError> {
		let rpc = self.inner.rpc_client();
		let payer = self.inner.payer();
		let benchmark_discriminator = instruction.data.first().copied();
		let mut transaction_signers: Vec<&dyn Signer> = Vec::with_capacity(signers.len() + 1);
		transaction_signers.push(payer);
		transaction_signers.extend_from_slice(signers);
		let blockhash = rpc
			.get_latest_blockhash()
			.map_err(|error| test_error("fetch latest blockhash", error))?;
		let message = Message::new(&[instruction], Some(&payer.pubkey()));
		let mut transaction = Transaction::new_unsigned(message);
		transaction
			.try_sign(&transaction_signers, blockhash)
			.map_err(|error| test_error("sign program transaction", error))?;

		if let (Some(program), Some(discriminator)) = (record_program, benchmark_discriminator) {
			record_compute_units(&rpc, &transaction, program, discriminator)?;
		}

		rpc.send_and_confirm_transaction(&transaction)
			.map_err(|error| test_error("execute program instruction", error))
	}

	/// Submit exact historical bytes and account metas to `program_id`.
	///
	/// # Errors
	///
	/// Returns an error when signing, submission, execution, or confirmation fails.
	pub fn send_historical_instruction(
		&self,
		program_id: Pubkey,
		fixture: &HistoricalInstruction,
		signers: &[&dyn Signer],
	) -> Result<Signature, TestError> {
		self.send_instruction_with_signers(fixture.instruction(program_id), signers)
	}

	/// Capture complete state for a set of existing accounts.
	///
	/// # Errors
	///
	/// Returns an error when any account cannot be fetched.
	pub fn snapshot_accounts(
		&self,
		addresses: &[Pubkey],
	) -> Result<Vec<AccountSnapshot>, TestError> {
		addresses
			.iter()
			.map(|address| {
				self.account(address).map(|account| {
					AccountSnapshot {
						address: *address,
						account,
					}
				})
			})
			.collect()
	}

	/// Compare current account state with a previously captured exact snapshot.
	///
	/// # Errors
	///
	/// Returns an error naming the first account whose lamports, data, owner,
	/// executable flag, or rent epoch changed.
	pub fn assert_accounts_match(&self, snapshots: &[AccountSnapshot]) -> Result<(), TestError> {
		for snapshot in snapshots {
			let current = self.account(&snapshot.address)?;
			if current != snapshot.account {
				return Err(test_error(
					"assert transaction rollback",
					format_args!("account {} changed after rejection", snapshot.address),
				));
			}
		}

		Ok(())
	}

	/// Require program execution to reject a historical request and prove exact
	/// rollback for every protected account.
	///
	/// # Errors
	///
	/// Returns an error when execution succeeds, fails before the program runs,
	/// or any protected account changes.
	pub fn expect_historical_rejection_with_rollback(
		&self,
		program_id: Pubkey,
		fixture: &HistoricalInstruction,
		protected_accounts: &[Pubkey],
		signers: &[&dyn Signer],
	) -> Result<(), TestError> {
		let snapshots = self.snapshot_accounts(protected_accounts)?;
		match self.send_historical_instruction(program_id, fixture, signers) {
			Ok(signature) => {
				return Err(test_error(
					"assert historical rejection",
					format_args!(
						"version {} unexpectedly succeeded as transaction {signature}",
						fixture.version
					),
				));
			}
			Err(error) if error.operation() == "execute program instruction" => {}
			Err(error) => return Err(error),
		}

		self.assert_accounts_match(&snapshots)
	}

	/// Fund an address inside the isolated Surfnet by transferring lamports
	/// from the pre-funded payer.
	///
	/// The transfer doubles as creation: the receiver starts as a system
	/// account holding the transferred balance, exactly like production
	/// funding flows.
	///
	/// # Errors
	///
	/// Returns an error when the transfer transaction cannot be signed,
	/// submitted, or confirmed.
	pub fn fund(&self, address: &Pubkey, lamports: u64) -> Result<Signature, TestError> {
		let instruction = self.transfer_instruction(address, lamports);

		self.send_instruction(instruction)
	}

	/// Fetch an account from the local RPC.
	///
	/// # Errors
	///
	/// Returns an error when the account does not exist or the local RPC fails.
	pub fn account(&self, address: &Pubkey) -> Result<Account, TestError> {
		self.inner
			.rpc_client()
			.get_account(address)
			.map_err(|error| test_error("fetch test account", error))
	}

	/// Fetch an address balance from the local RPC.
	///
	/// # Errors
	///
	/// Returns an error when the local RPC fails.
	pub fn balance(&self, address: &Pubkey) -> Result<u64, TestError> {
		self.inner
			.rpc_client()
			.get_balance(address)
			.map_err(|error| test_error("fetch test account balance", error))
	}

	/// Synchronously stop the owned Surfpool RPC servers and release their ports.
	///
	/// # Errors
	///
	/// Returns an error when both RPC servers do not confirm shutdown in time.
	pub fn stop(&mut self) -> Result<(), TestError> {
		self.inner
			.stop()
			.map_err(|error| test_error("stop offline Surfpool", error))
	}
}

fn record_compute_units(
	rpc: &solana_rpc_client::rpc_client::RpcClient,
	transaction: &Transaction,
	program: &str,
	discriminator: u8,
) -> Result<(), TestError> {
	let Some(output) = std::env::var_os("PINA_CU_RECORD_FILE") else {
		return Ok(());
	};
	let simulation = rpc
		.simulate_transaction(transaction)
		.map_err(|error| test_error("simulate program instruction", error))?;
	let compute_units = simulation
		.value
		.units_consumed
		.ok_or_else(|| test_error("record compute units", "simulation omitted compute units"))?;
	let record = serde_json::json!({
		"program": program,
		"discriminator": discriminator,
		"computeUnits": compute_units,
	});
	let _guard = BENCHMARK_RECORD_LOCK
		.lock()
		.unwrap_or_else(std::sync::PoisonError::into_inner);
	let mut file = OpenOptions::new()
		.create(true)
		.append(true)
		.open(output)
		.map_err(|error| test_error("open compute-unit record", error))?;
	serde_json::to_writer(&mut file, &record)
		.map_err(|error| test_error("write compute-unit record", error))?;
	writeln!(file).map_err(|error| test_error("write compute-unit record", error))?;

	Ok(())
}

fn test_error(operation: &'static str, error: impl std::fmt::Display) -> TestError {
	TestError {
		operation,
		message: error.to_string(),
	}
}

/// The system program, which owns lamports and account creation.
fn system_program_id() -> Pubkey {
	Pubkey::default()
}

fn artifact_from_env(program_id: &Pubkey) -> Result<(PathBuf, Option<String>), TestError> {
	if let Some(manifest_path) = std::env::var_os("PINA_CU_MANIFEST") {
		let manifest = std::fs::read_to_string(manifest_path)
			.map_err(|error| test_error("read compute-unit manifest", error))?;
		let manifest: serde_json::Value = serde_json::from_str(&manifest)
			.map_err(|error| test_error("parse compute-unit manifest", error))?;
		let entry = manifest.get(program_id.to_string()).ok_or_else(|| {
			test_error(
				"locate SBF program artifact",
				format_args!("benchmark manifest has no entry for {program_id}"),
			)
		})?;
		let artifact = entry
			.get("artifact")
			.and_then(serde_json::Value::as_str)
			.ok_or_else(|| test_error("locate SBF program artifact", "invalid artifact entry"))?;
		let program = entry
			.get("program")
			.and_then(serde_json::Value::as_str)
			.ok_or_else(|| test_error("locate SBF program artifact", "invalid program entry"))?;

		return artifact_path(OsString::from(artifact))
			.map(|path| (path, Some(program.to_owned())));
	}

	let artifact = std::env::var_os("PINA_SBF_ARTIFACT").ok_or_else(|| {
		test_error(
			"locate SBF program artifact",
			"PINA_SBF_ARTIFACT is not set; run this test with `pina test`",
		)
	})?;

	artifact_path(artifact).map(|path| (path, std::env::var("PINA_CU_PROGRAM").ok()))
}

fn artifact_path(artifact: OsString) -> Result<PathBuf, TestError> {
	let path = PathBuf::from(artifact);

	if !path.is_file() {
		return Err(test_error(
			"locate SBF program artifact",
			format_args!("missing file: {}", path.display()),
		));
	}

	Ok(path)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn preserves_error_operation_and_source() {
		let error = test_error("deploy", "missing artifact");

		assert_eq!(error.operation(), "deploy");
		assert_eq!(error.message(), "missing artifact");
		assert_eq!(error.to_string(), "deploy: missing artifact");
	}

	#[test]
	fn historical_fixtures_preserve_golden_bytes_and_account_metas() {
		let address = Pubkey::new_unique();
		let owner = Pubkey::new_unique();
		let account =
			HistoricalAccount::new(3, address, owner, [7, 3, 1, 2, 3]).with_lamports(42_000);
		assert_eq!(account.version(), 3);
		assert_eq!(account.address(), address);
		assert_eq!(account.account().owner, owner);
		assert_eq!(account.account().data, [7, 3, 1, 2, 3]);
		assert_eq!(account.account().lamports, 42_000);

		let metas = vec![
			AccountMeta::new(address, true),
			AccountMeta::new_readonly(owner, false),
		];
		let instruction = HistoricalInstruction::new(2, [9, 2, 4, 5], metas.clone());
		assert_eq!(instruction.version(), 2);
		assert_eq!(instruction.data(), [9, 2, 4, 5]);
		assert_eq!(instruction.accounts(), metas);

		let event = HistoricalEvent::new(1, [4, 1, 8, 7]);
		assert_eq!(event.version(), 1);
		assert_eq!(event.data(), [4, 1, 8, 7]);
	}

	#[test]
	fn owns_offline_lifecycle_and_reports_failed_operations() {
		run(async {
			let mut surfnet = OfflineSurfnet::start()
				.await
				.unwrap_or_else(|error| panic!("start offline Surfpool test instance: {error}"));
			assert_eq!(
				surfnet.payer(),
				Keypair::new_from_array(TEST_PAYER_SEED).pubkey(),
			);

			let missing = std::env::temp_dir().join("pina-test-missing-program.so");
			let program_id = Pubkey::new_unique();
			assert!(surfnet.deploy_program(program_id, &missing).is_err());
			assert!(surfnet.program_is_executable(&program_id).is_err());

			let invalid_instruction = Instruction::new_with_bytes(program_id, &[], Vec::new());
			assert!(surfnet.send_instruction(invalid_instruction).is_err());
			surfnet
				.stop()
				.unwrap_or_else(|error| panic!("stop offline Surfpool test instance: {error}"));
		});
	}

	#[test]
	fn installs_historical_state_and_proves_rejected_transaction_rollback() {
		run(async {
			let mut surfnet = OfflineSurfnet::start()
				.await
				.unwrap_or_else(|error| panic!("start offline Surfpool test instance: {error}"));
			let address = Pubkey::new_unique();
			let fixture = HistoricalAccount::new(0, address, Pubkey::new_unique(), [1, 0, 2, 3, 4])
				.with_lamports(1_000_000);
			surfnet
				.install_historical_account(&fixture)
				.unwrap_or_else(|error| panic!("install historical fixture: {error}"));
			assert_eq!(
				surfnet
					.account(&address)
					.unwrap_or_else(|error| panic!("fetch historical fixture: {error}")),
				*fixture.account()
			);

			let rejected = HistoricalInstruction::new(0, [9, 0], Vec::new());
			surfnet
				.expect_historical_rejection_with_rollback(
					Pubkey::new_unique(),
					&rejected,
					&[address],
					&[],
				)
				.unwrap_or_else(|error| panic!("prove rollback: {error}"));

			let snapshots = surfnet
				.snapshot_accounts(&[address])
				.unwrap_or_else(|error| panic!("snapshot fixture: {error}"));
			let changed = fixture.clone().with_lamports(2_000_000);
			surfnet
				.install_historical_account(&changed)
				.unwrap_or_else(|error| panic!("mutate historical fixture: {error}"));
			assert!(surfnet.assert_accounts_match(&snapshots).is_err());

			surfnet
				.stop()
				.unwrap_or_else(|error| panic!("stop offline Surfpool test instance: {error}"));
		});
	}
}
