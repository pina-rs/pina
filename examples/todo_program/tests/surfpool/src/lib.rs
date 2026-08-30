#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;
use program_under_test::TodoInstruction;

/// Seed prefix for todo PDAs, mirroring `TODO_SEED` in the program.
const TODO_SEED: &[u8] = b"todo";

fn pda_address(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(&[TODO_SEED, owner.as_ref()], program_id).0
}

fn bump_for(program_id: &Pubkey, owner: &Pubkey) -> u8 {
	Pubkey::try_find_program_address(&[TODO_SEED, owner.as_ref()], program_id)
		.expect("canonical bump for the todo PDA")
		.1
}

fn initialize_instruction(
	program: &ProgramTest,
	owner: &Pubkey,
	todo: &Pubkey,
	bump: u8,
	digest: [u8; 32],
) -> pina_test::Instruction {
	let mut data = vec![TodoInstruction::Initialize as u8, bump];
	data.extend_from_slice(&digest);

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*owner, true),
			AccountMeta::new(*todo, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Initialize stores the bounded-text layout: 1 discriminator byte, owner,
/// bump, completion flag, and the digest.
#[test]
#[ignore = "run with pina test"]
fn initializes_the_todo_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);
		let digest: [u8; 32] = core::array::from_fn(|index| index as u8);

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, digest,
			))
			.expect("execute Initialize");

		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(account.owner, program_id);
		assert_eq!(account.data.len(), 67, "TodoState layout is 67 bytes");
		assert_eq!(account.data[0], 1, "account discriminator is TodoState");
		// owner: 1..33
		assert_eq!(
			account.data[1..33],
			owner.to_bytes(),
			"stored owner matches"
		);
		assert_eq!(account.data[33], bump);
		assert_eq!(account.data[34], 0, "fresh todo is not completed");
		assert_eq!(account.data[35..67], digest, "digest round-trips on-chain");

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn toggling_updates_the_completion_flag() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);
		let digest = [7u8; 32];

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, digest,
			))
			.expect("execute Initialize");

		program
			.send_instruction(toggle_instruction(&program, &owner, &todo))
			.expect("first Toggle");
		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(
			account.data[34], 1,
			"todo is completed after the first toggle"
		);

		program
			.send_instruction(toggle_instruction(&program, &owner, &todo))
			.expect("second Toggle");
		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(account.data[34], 0, "second toggle restores the flag");

		program.stop().expect("stop isolated program test");
	});
}

fn toggle_instruction(
	program: &ProgramTest,
	owner: &Pubkey,
	todo: &Pubkey,
) -> pina_test::Instruction {
	program.instruction(
		&[TodoInstruction::ToggleCompleted as u8],
		vec![
			AccountMeta::new_readonly(*owner, true),
			AccountMeta::new(*todo, false),
		],
	)
}

#[test]
#[ignore = "run with pina test"]
fn digest_updates_replace_the_stored_value() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, [0u8; 32],
			))
			.expect("execute Initialize");

		let next_digest = [9u8; 32];
		let mut data = vec![TodoInstruction::UpdateDigest as u8];
		data.extend_from_slice(&next_digest);

		program
			.send_instruction(toggle_digest_update(&program, &owner, &todo, &data))
			.expect("execute UpdateDigest");

		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(account.data[35..], next_digest, "digest replaced on-chain");

		program.stop().expect("stop isolated program test");
	});
}

fn toggle_digest_update(
	program: &ProgramTest,
	owner: &Pubkey,
	todo: &Pubkey,
	data: &[u8],
) -> pina_test::Instruction {
	program.instruction(
		data,
		vec![
			AccountMeta::new_readonly(*owner, true),
			AccountMeta::new(*todo, false),
		],
	)
}

/// UpdateDigest with the wrong digest length must be a program error.
#[test]
#[ignore = "run with pina test"]
fn rejects_malformed_update_payloads() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, [1u8; 32],
			))
			.expect("execute Initialize");

		let too_short = [TodoInstruction::UpdateDigest as u8, 1, 2, 3];
		let error = program
			.send_instruction(toggle_digest_update(&program, &owner, &todo, &too_short))
			.expect_err("short digest payload must fail");

		assert_eq!(error.operation(), "execute program instruction");

		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(
			account.data[35..],
			[1u8; 32],
			"state survives the failed instruction"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// A second Initialize hits the create-account CPI on an existing account.
#[test]
#[ignore = "run with pina test"]
fn cannot_initialize_twice() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, [2u8; 32],
			))
			.expect("first Initialize");

		let error = program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, [2u8; 32],
			))
			.expect_err("second Initialize must fail");

		assert_eq!(error.operation(), "execute program instruction");

		// The original digest must survive the failed instruction.
		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(account.data[35..], [2u8; 32]);

		program.stop().expect("stop isolated program test");
	});
}

/// A non-owner signer cannot mutate someone else's todo.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_signer_who_is_not_the_stored_owner() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let owner = program.payer();
		let todo = pda_address(&program_id, &owner);
		let bump = bump_for(&program_id, &owner);

		program
			.send_instruction(initialize_instruction(
				&program, &owner, &todo, bump, [3u8; 32],
			))
			.expect("execute Initialize");

		let impostor = Keypair::new();
		program
			.fund(&impostor.pubkey(), 1_000_000_000)
			.expect("fund impostor");

		let mut payload = vec![TodoInstruction::UpdateDigest as u8];
		payload.extend_from_slice(&[9u8; 32]);

		let instruction = program.instruction(
			&payload,
			vec![
				AccountMeta::new_readonly(impostor.pubkey(), true),
				AccountMeta::new(todo, false),
			],
		);

		let error = program
			.send_with_signers(instruction, &[&impostor])
			.expect_err("a stranger must not update the digest");
		assert_eq!(error.operation(), "execute program instruction");

		let account = program.account(&todo).expect("fetch todo account");
		assert_eq!(account.data[35..], [3u8; 32], "digest is untouched");

		program.stop().expect("stop isolated program test");
	});
}
