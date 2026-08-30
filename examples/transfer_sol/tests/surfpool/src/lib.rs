#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;
use program_under_test::TransferInstruction;

const TRANSFER: u8 = TransferInstruction::CpiTransfer as u8;
const DIRECT: u8 = TransferInstruction::DirectTransfer as u8;

/// Exactly enough for rent-exempt system accounts created by the fixture.
const EPOCH_FUND: u64 = 1_000_000_000;

fn direct_transfer(
	program: &ProgramTest,
	sender: &Pubkey,
	recipient: &Pubkey,
	amount: u64,
	signed: bool,
) -> pina_test::Instruction {
	let data = {
		let mut bytes = vec![DIRECT];
		bytes.extend_from_slice(&amount.to_le_bytes());
		bytes
	};

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*sender, signed),
			AccountMeta::new(*recipient, false),
		],
	)
}

/// CPI transfers move lamports from the sender to an unfunded recipient.
#[test]
#[ignore = "run with pina test"]
fn cpi_transfer_moves_lamports() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		// Use a dedicated funded sender so fees do not blur the math.
		let sender = pina_test::Keypair::new();
		program
			.fund(&sender.pubkey(), EPOCH_FUND)
			.expect("fund sender");
		let sender_before = program.balance(&sender.pubkey()).expect("sender balance");

		let recipient = Pubkey::new_unique();
		let data = {
			let mut bytes = vec![TRANSFER];
			bytes.extend_from_slice(&500_000_000u64.to_le_bytes());
			bytes
		};
		let instruction = program.instruction(
			&data,
			vec![
				AccountMeta::new(sender.pubkey(), true),
				AccountMeta::new(recipient, false),
				AccountMeta::new_readonly(Pubkey::default(), false),
			],
		);

		program
			.send_with_signers(instruction, &[&sender])
			.expect("execute CpiTransfer");

		let recipient_balance = program.balance(&recipient).expect("recipient balance");
		assert_eq!(
			recipient_balance, 500_000_000,
			"recipient got exact lamports"
		);

		let sender_after = program.balance(&sender.pubkey()).expect("sender balance");
		assert_eq!(
			sender_after,
			sender_before - 500_000_000,
			"sender debited by exactly the transfer amount"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Asking for more than the sender holds fails with the custom error.
#[test]
#[ignore = "run with pina test"]
fn cpi_transfer_rejects_overdrafts() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let sender = pina_test::Keypair::new();
		program
			.fund(&sender.pubkey(), EPOCH_FUND)
			.expect("fund sender");
		let balance = program.balance(&sender.pubkey()).expect("sender balance");
		let recipient = Pubkey::new_unique();
		let data = {
			let mut bytes = vec![TRANSFER];
			bytes.extend_from_slice(&(balance + 1).to_le_bytes());
			bytes
		};
		let instruction = program.instruction(
			&data,
			vec![
				AccountMeta::new(sender.pubkey(), true),
				AccountMeta::new(recipient, false),
				AccountMeta::new_readonly(Pubkey::default(), false),
			],
		);

		let error = program
			.send_with_signers(instruction, &[&sender])
			.expect_err("transferring more than owned must fail");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("overdraft error: {}", error.message());

		let recipient_balance = program.balance(&recipient).expect("recipient balance");
		assert_eq!(recipient_balance, 0, "recipient untouched");

		program.stop().expect("stop isolated program test");
	});
}

/// Direct transfers require a program-owned sender. The fixture creates one
/// with a system create-account so the example's lamport-mutation path runs.
#[test]
#[ignore = "run with pina test"]
fn direct_transfer_moves_lamports_between_program_accounts() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		// Create a program-owned account the caller controls end to end.
		let sender = pina_test::Keypair::new();
		let space = 0u64;
		let create = {
			let mut data = vec![0u8, 0, 0, 0];
			data.extend_from_slice(&EPOCH_FUND.to_le_bytes());
			data.extend_from_slice(&space.to_le_bytes());
			data.extend_from_slice(program_id.as_ref());

			// The create-account instruction targets the SYSTEM program, not
			// this program.
			pina_test::Instruction::new_with_bytes(
				Pubkey::default(),
				&data,
				vec![
					AccountMeta::new(program.payer(), true),
					AccountMeta::new(sender.pubkey(), true),
					AccountMeta::new_readonly(program_id, false),
				],
			)
		};

		program
			.send_with_signers(create, &[&sender])
			.expect("create program-owned sender account");

		let recipient = Pubkey::new_unique();
		program
			.fund(&recipient, EPOCH_FUND)
			.expect("fund recipient");

		let amount = 500_000_000u64;
		let sender_before = program.balance(&sender.pubkey()).expect("sender balance");
		program
			.send_with_signers(
				direct_transfer(&program, &sender.pubkey(), &recipient, amount, true),
				&[&sender],
			)
			.expect("execute DirectTransfer");

		let sender_after = program.balance(&sender.pubkey()).expect("sender balance");
		let recipient_after = program.balance(&recipient).expect("recipient balance");
		assert_eq!(sender_after, sender_before - amount);
		assert_eq!(recipient_after, EPOCH_FUND + amount);

		program.stop().expect("stop isolated program test");
	});
}

/// A sender not owned by the program is refused before any balance moves.
#[test]
#[ignore = "run with pina test"]
fn direct_transfer_rejects_non_program_owners() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let outsider = Pubkey::new_unique();
		program.fund(&outsider, EPOCH_FUND).expect("fund outsider");
		let recipient = Pubkey::new_unique();
		program
			.fund(&recipient, EPOCH_FUND)
			.expect("fund recipient");

		let instruction = direct_transfer(&program, &outsider, &recipient, 1_000_000, false);
		let error = program
			.send_instruction(instruction)
			.expect_err("only program-owned accounts may direct transfer");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("wrong owner direct transfer error: {}", error.message());

		let recipient_after = program.balance(&recipient).expect("recipient balance");
		assert_eq!(recipient_after, EPOCH_FUND, "recipient untouched");

		program.stop().expect("stop isolated program test");
	});
}
