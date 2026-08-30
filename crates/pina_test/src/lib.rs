//! Focused, host-side Surfpool integration test support for Pina programs.
//!
//! [`OfflineSurfnet`] always starts with upstream RPC access disabled, owns its
//! dynamically allocated ports, and requests synchronous shutdown. Its inner
//! Surfnet also requests shutdown from `Drop`, including during a panic.

use std::ffi::OsString;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

pub use solana_account::Account;
pub use solana_instruction::AccountMeta;
pub use solana_instruction::Instruction;
pub use solana_keypair::Keypair;
use solana_message::Message;
pub use solana_pubkey::Pubkey;
pub use solana_signature::Signature;
pub use solana_signer::Signer;
use solana_transaction::Transaction;
use surfpool_sdk::Surfnet;
use surfpool_sdk::cheatcodes::builders::DeployProgram;

/// Run an async integration-test body on a dedicated Tokio runtime.
///
/// # Panics
///
/// Panics if Tokio cannot construct the test runtime.
pub fn run<F>(future: F) -> F::Output
where
	F: Future,
{
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.unwrap_or_else(|error| panic!("build Pina integration-test runtime: {error}"))
		.block_on(future)
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

/// A deployed Pina program running in its own isolated Surfpool instance.
///
/// `pina test` sets `PINA_SBF_ARTIFACT` before it runs the dedicated Surfpool
/// test package. [`ProgramTest::start`] consumes that artifact path, deploys the
/// program, and leaves tests to focus on instructions and state assertions.
pub struct ProgramTest {
	program_id: Pubkey,
	surfnet: OfflineSurfnet,
}

impl ProgramTest {
	/// Start an offline Surfnet and deploy the artifact supplied by `pina test`.
	///
	/// # Errors
	///
	/// Returns an error when `PINA_SBF_ARTIFACT` is missing, does not name a file,
	/// or the Surfnet cannot start and deploy the program.
	pub async fn start(program_id: Pubkey) -> Result<Self, TestError> {
		let artifact = artifact_from_env()?;

		Self::start_with_artifact(program_id, &artifact).await
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
		self.surfnet.send_instruction(instruction)
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
		self.surfnet
			.send_instruction_with_signers(instruction, signers)
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

impl OfflineSurfnet {
	/// Start an isolated Surfpool instance without contacting an upstream RPC.
	///
	/// # Errors
	///
	/// Returns an error when Surfpool cannot allocate ports or start its runtime.
	pub async fn start() -> Result<Self, TestError> {
		let inner = Surfnet::builder()
			.offline(true)
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
		self.send_instruction_with_signers(instruction, &[])
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
		let rpc = self.inner.rpc_client();
		let payer = self.inner.payer();
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

		rpc.send_and_confirm_transaction(&transaction)
			.map_err(|error| test_error("execute program instruction", error))
	}

	/// Fund an address inside the isolated Surfnet.
	///
	/// # Errors
	///
	/// Returns an error when the local RPC rejects or cannot confirm the airdrop.
	pub fn fund(&self, address: &Pubkey, lamports: u64) -> Result<Signature, TestError> {
		let rpc = self.inner.rpc_client();
		let signature = rpc
			.request_airdrop(address, lamports)
			.map_err(|error| test_error("request test account funding", error))?;
		let confirmed = rpc
			.confirm_transaction(&signature)
			.map_err(|error| test_error("confirm test account funding", error))?;

		if !confirmed {
			return Err(test_error(
				"confirm test account funding",
				"transaction was not confirmed",
			));
		}

		Ok(signature)
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

fn test_error(operation: &'static str, error: impl std::fmt::Display) -> TestError {
	TestError {
		operation,
		message: error.to_string(),
	}
}

fn artifact_from_env() -> Result<PathBuf, TestError> {
	let artifact = std::env::var_os("PINA_SBF_ARTIFACT").ok_or_else(|| {
		test_error(
			"locate SBF program artifact",
			"PINA_SBF_ARTIFACT is not set; run this test with `pina test`",
		)
	})?;

	artifact_path(artifact)
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
	fn owns_offline_lifecycle_and_reports_failed_operations() {
		run(async {
			let mut surfnet = OfflineSurfnet::start()
				.await
				.unwrap_or_else(|error| panic!("start offline Surfpool test instance: {error}"));
			assert_ne!(surfnet.payer(), Pubkey::default());

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
}
