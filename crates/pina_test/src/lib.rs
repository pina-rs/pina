//! Focused, host-side Surfpool integration test support for Pina programs.
//!
//! [`OfflineSurfnet`] always starts with upstream RPC access disabled, owns its
//! dynamically allocated ports, and requests synchronous shutdown. Its inner
//! Surfnet also requests shutdown from `Drop`, including during a panic.

use std::future::Future;
use std::path::Path;

pub use solana_instruction::AccountMeta;
pub use solana_instruction::Instruction;
use solana_message::Message;
pub use solana_pubkey::Pubkey;
use solana_transaction::Transaction;
use surfpool_sdk::Signer;
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
	pub fn send_instruction(&self, instruction: Instruction) -> Result<(), TestError> {
		let rpc = self.inner.rpc_client();
		let payer = self.inner.payer();
		let blockhash = rpc
			.get_latest_blockhash()
			.map_err(|error| test_error("fetch latest blockhash", error))?;
		let message = Message::new(&[instruction], Some(&payer.pubkey()));
		let transaction = Transaction::new(&[payer], message, blockhash);

		rpc.send_and_confirm_transaction(&transaction)
			.map(|_| ())
			.map_err(|error| test_error("execute program instruction", error))
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn preserves_error_operation_and_source() {
		let error = test_error("deploy", "missing artifact");

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
