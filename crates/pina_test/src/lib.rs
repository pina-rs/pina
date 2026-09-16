//! Focused, host-side Surfpool integration test support for Pina programs.
//!
//! [`OfflineSurfnet`] always starts with upstream RPC access disabled, owns its
//! dynamically allocated ports, and requests synchronous shutdown. Its inner
//! Surfnet also requests shutdown from `Drop`, including during a panic.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write as _;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
pub use solana_account::Account;
use solana_client::client_error::ClientError;
pub use solana_instruction::AccountMeta;
pub use solana_instruction::Instruction;
pub use solana_instruction::error::InstructionError;
pub use solana_keypair::Keypair;
use solana_message::Message;
use solana_message::VersionedMessage;
use solana_message::v1;
/// The compute budget a transaction carries inside its message.
///
/// A v1 message states its compute unit limit, loaded accounts data size
/// limit, heap size, and priority fee directly instead of routing them through
/// compute-budget instructions. Build one from [`default_budget`] to match
/// legacy behavior and override only what a test needs.
pub use solana_message::v1::TransactionConfig;
pub use solana_pubkey::Pubkey;
pub use solana_rent::Rent;
use solana_rpc_client::api::request::RpcRequest;
use solana_rpc_client::rpc_client::SerializableTransaction;
pub use solana_signature::Signature;
pub use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_transaction::versioned::VersionedTransaction;
pub use solana_transaction_error::TransactionError;
use surfpool_sdk::Surfnet;
use surfpool_sdk::cheatcodes::builders::DeployProgram;
use surfpool_sdk::cheatcodes::builders::SetAccount;

/// Compute unit limit that matches the runtime's legacy default.
pub const DEFAULT_COMPUTE_UNIT_LIMIT: u32 = 200_000;

/// Loaded accounts data size limit matching the runtime's legacy default of
/// 64 MiB.
pub const DEFAULT_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 64 * 1024 * 1024;

/// The compute budget a transaction needs to behave like a legacy one.
///
/// Legacy and v0 transactions declare their limits with compute-budget
/// instructions, and the runtime supplies a default when none are present. V1
/// moves those limits into the message, so a transaction that wants the same
/// budget has to state it. This applies the runtime's legacy defaults so a
/// test can switch formats without reasoning about compute.
#[must_use]
pub const fn default_budget() -> TransactionConfig {
	TransactionConfig::empty()
		.with_compute_unit_limit(DEFAULT_COMPUTE_UNIT_LIMIT)
		.with_loaded_accounts_data_size_limit(DEFAULT_LOADED_ACCOUNTS_DATA_SIZE_LIMIT)
}

/// What one simulated transaction produced.
struct SimulationOutcome {
	/// Program log lines, in the order the runtime wrote them.
	logs: Vec<String>,
	/// Compute units the simulation charged.
	units: Option<u64>,
	/// Why the transaction failed, when it did.
	error: Option<serde_json::Value>,
}

/// The transaction format a test submits.
#[derive(Clone, Debug)]
pub enum TransactionFormat {
	/// The original format. The compute budget comes from compute-budget
	/// instructions the payer adds, and the message is limited to 1,232 bytes.
	Legacy,
	/// SIMD-0385. The compute budget is inline in the message, which raises
	/// the limit to 4,096 bytes but drops address lookup tables.
	V1(TransactionConfig),
}

static BENCHMARK_RECORD_LOCK: Mutex<()> = Mutex::new(());
const TEST_PAYER_SEED: [u8; 32] = [0xA5; 32];

/// How long [`OfflineSurfnet::stop`] waits for both RPC listeners to close.
///
/// `Surfnet::stop` only confirms shutdown within five seconds, which shared CI
/// runners exceed while several benchmark processes drain at once.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Delay between RPC listener probes while a Surfpool instance drains.
const SHUTDOWN_PROBE_INTERVAL: Duration = Duration::from_millis(250);

/// How long a single probe waits for a connection attempt to resolve.
const SHUTDOWN_PROBE_TIMEOUT: Duration = Duration::from_millis(250);

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
	#[source]
	client_error: Option<ClientError>,
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

	/// Complete RPC client error retained for transaction execution failures.
	#[must_use]
	pub const fn client_error(&self) -> Option<&ClientError> {
		self.client_error.as_ref()
	}

	/// Transaction-level failure extracted from the retained RPC client error.
	#[must_use]
	pub fn transaction_error(&self) -> Option<TransactionError> {
		self.client_error
			.as_ref()
			.and_then(ClientError::get_transaction_error)
	}
}

/// Assert that a program rejected an instruction with a specific custom code.
///
/// This is the assertion a test usually wants: not "something failed" but "the
/// program refused this request for the reason it was supposed to". It reads
/// the structured [`TransactionError`] retained by [`ProgramTest`] rather than
/// matching rendered text, so it is unaffected by how the runtime words a
/// failure and cannot confuse the expected error with a different one that
/// happens to share digits.
///
/// The expected code accepts anything that converts into `u32`, so a generated
/// `#[error]` enum, a [`core::num::TryFromIntError`]-checked value, or a plain
/// `u32` all work.
///
/// # Panics
///
/// Panics with a message naming the expected and actual errors when the
/// transaction carried no error, carried a non-custom instruction error, or
/// carried a different custom code.
#[track_caller]
pub fn assert_custom_error(error: &TestError, expected: impl Into<u32>) {
	let expected = expected.into();
	let Some(transaction_error) = error.transaction_error() else {
		panic!(
			"expected the program to fail with custom error {expected}, but the transaction \
			 carried no transaction error: {error}"
		);
	};

	let actual = match &transaction_error {
		TransactionError::InstructionError(_index, InstructionError::Custom(code)) => *code,
		other => {
			panic!(
				"expected the program to fail with custom error {expected}, but it failed with \
				 {other:?}"
			)
		}
	};

	assert_eq!(
		actual, expected,
		"expected custom error {expected}, got custom error {actual}"
	);
}

/// Decode a fetched account's data with the program's own reader.
///
/// Tests otherwise reach into raw bytes — asserting `account.data[0]` for a
/// discriminator, `account.data[2..]` for a field — which silently keeps
/// passing against the wrong layout and cannot tell a decode failure from a
/// value assertion. Passing the program's generated reader here means a layout
/// change fails the test at the decode, and the error the program would return
/// stays available to assert on.
///
/// # Errors
///
/// Returns whatever `decode` returns, so the caller asserts on the program's
/// own error type instead of a flattened test error.
pub fn with_account_bytes<T, E>(
	account: &Account,
	decode: impl FnOnce(&[u8]) -> Result<T, E>,
) -> Result<T, E> {
	decode(&account.data)
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

	/// Simulate one instruction without submitting it and return its logs.
	///
	/// Because the transaction is never sent, this reads the `Program data:`
	/// records a program emitted even when it also returned an error.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, signing, or simulation fails.
	pub fn simulate_logs(
		&self,
		data: &[u8],
		accounts: Vec<AccountMeta>,
	) -> Result<Vec<String>, TestError> {
		self.surfnet
			.simulate_program_logs(self.program_id, data, accounts)
	}

	/// Build and sign a transaction addressed to the deployed program.
	///
	/// Use this to prove a program accepts a wire format, or to carry an
	/// instruction whose data does not fit
	/// [`TransactionFormat::Legacy`]'s 1,232-byte limit.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, message compilation, or
	/// signing fails.
	pub fn sign_transaction(
		&self,
		data: &[u8],
		accounts: Vec<AccountMeta>,
		format: &TransactionFormat,
	) -> Result<VersionedTransaction, TestError> {
		self.surfnet
			.sign_transaction(self.instruction(data, accounts), format)
	}

	/// Simulate a transaction and return the program log lines it produced.
	///
	/// # Errors
	///
	/// Returns an error when message construction, signing, or simulation fails.
	pub fn simulate_transaction_logs(
		&self,
		data: &[u8],
		accounts: Vec<AccountMeta>,
		format: &TransactionFormat,
	) -> Result<Vec<String>, TestError> {
		self.surfnet
			.simulate_transaction_logs(self.program_id, data, accounts, format)
	}

	/// Submit one instruction in `format` and confirm it.
	///
	/// Records compute units for the benchmark harness exactly like
	/// [`Self::send`], so a non-legacy path is measured rather than skipped.
	///
	/// # Errors
	///
	/// Returns an error when message construction, signing, submission, or
	/// confirmation fails.
	pub fn send_transaction(
		&self,
		data: &[u8],
		accounts: Vec<AccountMeta>,
		format: &TransactionFormat,
	) -> Result<Signature, TestError> {
		self.surfnet.send_program_transaction(
			self.program_id,
			self.benchmark_program.as_deref(),
			data,
			accounts,
			format,
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
		// panics. Drain synchronously here before the unwinder continues, with
		// the same retry budget tests get so a panic cannot leak bound ports.
		let _ = self.stop();
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

	/// Build and sign a v1 transaction for one instruction.
	///
	/// Simulate one instruction addressed to `program_id` and return its logs.
	///
	/// The transaction is never submitted, so a failing program still yields
	/// the log lines it produced before the error.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, signing, or simulation fails.
	pub fn simulate_program_logs(
		&self,
		program_id: Pubkey,
		data: &[u8],
		accounts: Vec<AccountMeta>,
	) -> Result<Vec<String>, TestError> {
		let rpc = self.inner.rpc_client();
		let payer = self.inner.payer();
		let blockhash = rpc
			.get_latest_blockhash()
			.map_err(|error| test_error("fetch latest blockhash", error))?;
		let message = Message::new(
			&[Instruction::new_with_bytes(program_id, data, accounts)],
			Some(&payer.pubkey()),
		);
		let mut transaction = Transaction::new_unsigned(message);
		transaction
			.try_sign(&[payer], blockhash)
			.map_err(|error| test_error("sign program transaction", error))?;
		let simulation = rpc
			.simulate_transaction(&transaction)
			.map_err(|error| test_error("simulate program instruction", error))?;

		Ok(simulation.value.logs.unwrap_or_default())
	}

	/// The message carries its compute budget inline and appends signatures
	/// after the message, so a v1 transaction can hold up to
	/// [`solana_message::v1::MAX_TRANSACTION_SIZE`] bytes — more than three
	/// times a legacy transaction — at the cost of address lookup tables,
	/// which v1 does not support.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, message compilation, or
	/// signing fails.
	pub fn sign_transaction(
		&self,
		instruction: Instruction,
		format: &TransactionFormat,
	) -> Result<VersionedTransaction, TestError> {
		let rpc = self.inner.rpc_client();
		let payer = self.inner.payer();
		let blockhash = rpc
			.get_latest_blockhash()
			.map_err(|error| test_error("fetch latest blockhash", error))?;
		let message = match format {
			TransactionFormat::Legacy => {
				VersionedMessage::Legacy(Message::new(&[instruction], Some(&payer.pubkey())))
			}
			TransactionFormat::V1(config) => {
				VersionedMessage::V1(
					v1::Message::try_compile_with_config(
						&payer.pubkey(),
						&[instruction],
						blockhash,
						*config,
					)
					.map_err(|error| test_error("compile transaction message", error))?,
				)
			}
		};
		let signers: Vec<&dyn Signer> = vec![payer];
		VersionedTransaction::try_new(message, &signers)
			.map_err(|error| test_error("sign program transaction", error))
	}

	/// Simulate a transaction and return the program log lines it produced.
	///
	/// # Errors
	///
	/// Returns an error when message construction, signing, or simulation
	/// fails, or when the simulated transaction fails.
	pub fn simulate_transaction_logs(
		&self,
		program_id: Pubkey,
		data: &[u8],
		accounts: Vec<AccountMeta>,
		format: &TransactionFormat,
	) -> Result<Vec<String>, TestError> {
		let SimulationOutcome {
			logs,
			units: _,
			error,
		} = self.simulate(
			Instruction::new_with_bytes(program_id, data, accounts),
			format,
		)?;

		if let Some(error) = error {
			return Err(test_error(
				"simulate program instruction",
				format_args!("{error}\n{}", logs.join("\n")),
			));
		}

		Ok(logs)
	}

	/// Simulate a transaction, returning its logs, consumed compute units, and
	/// the failure when the runtime rejected it.
	///
	/// V1 has no serde representation, so its transactions are submitted in
	/// their own wire form through a generic request; the other formats use the
	/// typed client.
	fn simulate(
		&self,
		instruction: Instruction,
		format: &TransactionFormat,
	) -> Result<SimulationOutcome, TestError> {
		let transaction = self.sign_transaction(instruction, format)?;

		if let VersionedMessage::V1(_) = &transaction.message {
			let encoding = BASE64_STANDARD.encode(wire_bytes(&transaction)?);
			let response: serde_json::Value = self
				.inner
				.rpc_client()
				.send(
					RpcRequest::SimulateTransaction,
					serde_json::json!([
						encoding,
						{ "encoding": "base64", "sigVerify": false },
					]),
				)
				.map_err(|error| test_error("simulate program instruction", error))?;

			return Ok(SimulationOutcome {
				logs: logs(&response),
				units: units(&response),
				error: simulation_error(&response),
			});
		}

		let simulation = self
			.inner
			.rpc_client()
			.simulate_transaction(&transaction)
			.map_err(|error| test_error("simulate program instruction", error))?;

		Ok(SimulationOutcome {
			logs: simulation.value.logs.unwrap_or_default(),
			units: simulation.value.units_consumed,
			error: simulation.value.err.map(|error| serde_json::json!(error)),
		})
	}

	/// Submit a signed transaction.
	///
	/// V1 is submitted in its own wire form because it has no serde
	/// representation for the typed client to encode.
	///
	/// # Errors
	///
	/// Returns an error when submission fails.
	pub fn submit_transaction(
		&self,
		transaction: &VersionedTransaction,
	) -> Result<Signature, TestError> {
		match &transaction.message {
			VersionedMessage::V1(_) => {
				let encoding = BASE64_STANDARD.encode(wire_bytes(transaction)?);
				let signature: String = self
					.inner
					.rpc_client()
					.send(
						RpcRequest::SendTransaction,
						serde_json::json!([encoding, { "encoding": "base64" }]),
					)
					.map_err(execution_error)?;
				let signature = signature
					.parse()
					.map_err(|error| test_error("parse transaction signature", error))?;

				if !self
					.inner
					.rpc_client()
					.confirm_transaction(&signature)
					.map_err(|error| test_error("confirm transaction", error))?
				{
					return Err(test_error(
						"confirm transaction",
						"the transaction was not confirmed",
					));
				}

				Ok(signature)
			}
			_ => {
				self.inner
					.rpc_client()
					.send_and_confirm_transaction(transaction)
					.map_err(execution_error)
			}
		}
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

	/// Build, optionally measure, and submit one instruction as a v1 transaction.
	///
	/// # Errors
	///
	/// Returns an error when blockhash retrieval, message compilation, signing,
	/// submission, or confirmation fails.
	fn send_program_transaction(
		&self,
		program_id: Pubkey,
		benchmark_program: Option<&str>,
		data: &[u8],
		accounts: Vec<AccountMeta>,
		format: &TransactionFormat,
	) -> Result<Signature, TestError> {
		let transaction = self.sign_transaction(
			Instruction::new_with_bytes(program_id, data, accounts.clone()),
			format,
		)?;

		if let (Some(program), Some(discriminator)) = (benchmark_program, data.first().copied()) {
			let SimulationOutcome {
				logs, units, error, ..
			} = self.simulate(
				Instruction::new_with_bytes(program_id, data, accounts),
				format,
			)?;
			if let Some(error) = error {
				return Err(test_error(
					"simulate program instruction",
					format_args!("{error}\n{}", logs.join("\n")),
				));
			}
			let compute_units = units.ok_or_else(|| {
				test_error("record compute units", "simulation omitted compute units")
			})?;
			record_units(program, discriminator, compute_units)?;
		}

		self.submit_transaction(&transaction)
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
			.map_err(execution_error)
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
			Err(error)
				if matches!(
					error.transaction_error(),
					Some(TransactionError::InstructionError(..))
				) => {}
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
	/// The SDK waits five seconds for both RPC servers to acknowledge the
	/// terminate command. A loaded CI runner can exceed that window while the
	/// server threads drain, so retry until the SDK confirms shutdown or both
	/// listener ports stop accepting connections.
	///
	/// # Errors
	///
	/// Returns an error when both RPC servers do not confirm shutdown in time.
	pub fn stop(&mut self) -> Result<(), TestError> {
		let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
		// Each attempt reports how many servers acknowledged shutdown, so the
		// first failure carries the most diagnostic context.
		let mut first_error = None;

		loop {
			match self.inner.stop() {
				Ok(()) => return Ok(()),
				Err(error) => {
					first_error.get_or_insert_with(|| error.to_string());
				}
			}

			if self.rpc_listeners_are_closed() {
				return Ok(());
			}

			if Instant::now() >= deadline {
				let message =
					first_error.unwrap_or_else(|| "shutdown was not confirmed".to_owned());

				return Err(test_error("stop offline Surfpool", message));
			}

			std::thread::sleep(SHUTDOWN_PROBE_INTERVAL);
		}
	}

	/// Return whether neither RPC listener accepts connections any more.
	fn rpc_listeners_are_closed(&self) -> bool {
		[self.inner.rpc_url(), self.inner.ws_url()]
			.into_iter()
			.all(rpc_listener_is_closed)
	}
}

/// Return whether the listener behind a `scheme://host:port` URL is gone.
///
/// A refused connection proves the port stopped accepting connections. Parsing
/// failures and timeouts stay pessimistic so `stop` keeps waiting.
fn rpc_listener_is_closed(url: &str) -> bool {
	let Some(address) = url
		.rsplit_once("://")
		.and_then(|(_, address)| address.parse::<SocketAddr>().ok())
	else {
		return false;
	};

	match TcpStream::connect_timeout(&address, SHUTDOWN_PROBE_TIMEOUT) {
		Ok(_) => false,
		Err(error) => error.kind() == std::io::ErrorKind::ConnectionRefused,
	}
}

/// Encode a signed v1 transaction in the format the runtime deserializes.
///
/// A v1 message has no serde representation — the serialization contract is
/// `wincode`, so the RPC client's `Serialize`-based transport would send bytes
/// no node can parse. Encoding the message and appending the header-sized
/// signature array produces the wire form, which callers then submit
/// base64-encoded.
fn logs(response: &serde_json::Value) -> Vec<String> {
	response
		.pointer("/value/logs")
		.and_then(serde_json::Value::as_array)
		.map(|entries| {
			entries
				.iter()
				.filter_map(serde_json::Value::as_str)
				.map(str::to_owned)
				.collect::<Vec<_>>()
		})
		.unwrap_or_default()
}

fn units(response: &serde_json::Value) -> Option<u64> {
	response
		.pointer("/value/unitsConsumed")
		.and_then(serde_json::Value::as_u64)
}

fn simulation_error(response: &serde_json::Value) -> Option<serde_json::Value> {
	response
		.pointer("/value/err")
		.filter(|error| !error.is_null())
		.cloned()
}

/// Encode a signed v1 transaction in the format the runtime deserializes.
///
/// A v1 message has no serde representation — its serialization contract is
/// `wincode` — so the typed client's `Serialize`-based transport would send
/// bytes no node can parse. The message and the header-sized signature array
/// are concatenated here instead, and the result is submitted base64-encoded.
fn wire_bytes(transaction: &VersionedTransaction) -> Result<Vec<u8>, TestError> {
	let VersionedMessage::V1(message) = &transaction.message else {
		return Err(test_error(
			"encode v1 transaction",
			"transaction message is not v1",
		));
	};

	let mut wire = VersionedMessage::V1(message.clone()).serialize();
	for signature in &transaction.signatures {
		wire.extend_from_slice(signature.as_ref());
	}

	Ok(wire)
}

/// Record one measured compute-unit figure for the benchmark harness.
fn record_units(program: &str, discriminator: u8, compute_units: u64) -> Result<(), TestError> {
	write_compute_units(program, discriminator, compute_units)
}

/// Record the compute units a legacy-encoded transaction consumed.
fn record_compute_units(
	rpc: &solana_rpc_client::rpc_client::RpcClient,
	transaction: &impl SerializableTransaction,
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

	drop(output);
	write_compute_units(program, discriminator, compute_units)
}

fn write_compute_units(
	program: &str,
	discriminator: u8,
	compute_units: u64,
) -> Result<(), TestError> {
	let Some(output) = std::env::var_os("PINA_CU_RECORD_FILE") else {
		return Ok(());
	};
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
		client_error: None,
	}
}

/// Build a `TestError` carrying one instruction error, for the in-crate tests.
///
/// The public surface extracts `TransactionError` from an RPC `ClientError`,
/// which cannot be constructed cheaply, so the extraction path is exercised
/// through this narrow constructor instead.
#[cfg(test)]
fn execution_error_with(instruction_error: InstructionError) -> TestError {
	TestError {
		operation: "execute program instruction",
		message: format!("{instruction_error:?}"),
		client_error: Some(ClientError::from(TransactionError::InstructionError(
			0,
			instruction_error,
		))),
	}
}

/// Read the panic payload the standard hook would have printed.
#[cfg(test)]
fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
	panic
		.downcast_ref::<String>()
		.cloned()
		.or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
		.unwrap_or_default()
}

fn execution_error(error: ClientError) -> TestError {
	TestError {
		operation: "execute program instruction",
		message: error.to_string(),
		client_error: Some(error),
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

	/// A custom code matches exactly.
	#[test]
	fn custom_error_assertion_accepts_the_expected_code() {
		let error = execution_error_with(InstructionError::Custom(6000));

		assert_custom_error(&error, 6000u32);
	}

	/// A different code fails, and the message names both so the failure is
	/// diagnosable without re-running the test.
	#[test]
	fn custom_error_assertion_rejects_a_different_code() {
		let error = execution_error_with(InstructionError::Custom(6000));

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assert_custom_error(&error, 6001u32);
		}))
		.expect_err("a mismatched code must fail the assertion");
		let message = panic_message(panic);

		assert!(
			message.contains("6001") && message.contains("6000"),
			"the failure must name expected and actual: {message}"
		);
	}

	/// A standard `InstructionError` is reported as itself rather than being
	/// mistaken for a custom code.
	#[test]
	fn custom_error_assertion_rejects_a_standard_error() {
		let error = execution_error_with(InstructionError::InvalidArgument);

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assert_custom_error(&error, 6000u32);
		}))
		.expect_err("a non-custom error must fail the assertion");
		let message = panic_message(panic);

		assert!(
			message.contains("InvalidArgument"),
			"the failure must report what was actually returned: {message}"
		);
	}

	/// An error carrying no transaction detail fails with a clear message
	/// instead of silently passing.
	#[test]
	fn custom_error_assertion_requires_transaction_detail() {
		let error = test_error("execute program instruction", "no detail");

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assert_custom_error(&error, 6000u32);
		}))
		.expect_err("missing detail must fail the assertion");
		let message = panic_message(panic);

		assert!(
			message.contains("no transaction error"),
			"the failure must say the detail was missing: {message}"
		);
	}

	/// The framework's reserved band is matched exactly, including codes above
	/// the range a user `#[error]` enum may occupy.
	#[test]
	fn custom_error_assertion_accepts_a_reserved_code() {
		let error = execution_error_with(InstructionError::Custom(0xFFFF_FFF8));

		assert_custom_error(&error, 0xFFFF_FFF8u32);
	}

	/// Decoding through the helper yields the value the decoder produced.
	#[test]
	fn account_data_helper_passes_the_fetched_bytes_to_the_decoder() {
		let account = solana_account::Account {
			lamports: 1,
			data: vec![7, 8, 9],
			owner: Pubkey::default(),
			executable: false,
			rent_epoch: 0,
		};

		let decoded = with_account_bytes(&account, |data| Ok::<u8, u32>(data.iter().sum::<u8>()))
			.expect("decode succeeds");

		assert_eq!(decoded, 24);
	}

	/// A decoder rejection is returned as the decoder's own error type rather
	/// than being flattened into a `TestError`, so a test can assert on the
	/// exact variant the program returned.
	#[test]
	fn account_data_helper_propagates_the_decoder_error() {
		let account = solana_account::Account {
			lamports: 1,
			data: vec![0; 4],
			owner: Pubkey::default(),
			executable: false,
			rent_epoch: 0,
		};

		let error = with_account_bytes(&account, |_data| Err::<u8, u32>(9))
			.expect_err("a rejected decode must surface the program error");

		assert_eq!(error, 9u32);
	}

	#[test]
	fn preserves_error_operation_and_source() {
		let error = test_error("deploy", "missing artifact");

		assert_eq!(error.operation(), "deploy");
		assert_eq!(error.message(), "missing artifact");
		assert_eq!(error.to_string(), "deploy: missing artifact");
	}

	/// A v1 transaction carries instruction data that cannot fit a legacy one.
	///
	/// This is the property an example built for the larger limit depends on:
	/// the same signed transaction that v1 accepts is too large for the 1,232
	/// bytes a legacy transaction allows, and the wire form round-trips so a
	/// node reconstructs the signed message exactly.
	#[test]
	fn wire_bytes_exceed_the_legacy_limit_and_round_trip() {
		let payer = Keypair::new_from_array(TEST_PAYER_SEED);
		let mut data = vec![0_u8; 3_000];
		data[0] = 7;
		let instruction = Instruction::new_with_bytes(Pubkey::new_unique(), &data, Vec::new());
		let message = v1::Message::try_compile_with_config(
			&payer.pubkey(),
			&[instruction],
			solana_hash::Hash::default(),
			default_budget(),
		)
		.unwrap_or_else(|error| panic!("compile an oversized v1 message: {error}"));
		let signers: Vec<&dyn Signer> = vec![&payer];
		let transaction = VersionedTransaction::try_new(VersionedMessage::V1(message), &signers)
			.unwrap_or_else(|error| panic!("sign an oversized v1 transaction: {error}"));

		let wire = wire_bytes(&transaction)
			.unwrap_or_else(|error| panic!("encode v1 wire bytes: {error}"));
		assert!(
			1_232 < wire.len() && wire.len() <= solana_message::v1::MAX_TRANSACTION_SIZE,
			"{} bytes must exceed the legacy limit and fit the v1 one",
			wire.len()
		);

		let encoded = BASE64_STANDARD.encode(&wire);
		let decoded = BASE64_STANDARD
			.decode(&encoded)
			.unwrap_or_else(|error| panic!("base64 survives the round trip: {error}"));
		assert_eq!(decoded, wire);
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
	fn detects_closed_rpc_listeners() {
		let listener = std::net::TcpListener::bind("127.0.0.1:0")
			.unwrap_or_else(|error| panic!("bind probe listener: {error}"));
		let address = listener
			.local_addr()
			.unwrap_or_else(|error| panic!("read probe listener address: {error}"));
		let url = format!("http://{address}");

		assert!(!rpc_listener_is_closed(&url));

		drop(listener);

		assert!(rpc_listener_is_closed(&url));
		assert!(!rpc_listener_is_closed("not-a-url"));
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
					system_program_id(),
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
