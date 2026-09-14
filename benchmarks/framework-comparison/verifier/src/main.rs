//! Measures the compute units a compiled SBF program spends on one instruction
//! flow, and proves the flow still works.
//!
//! The comparison harness compiles the same hello-world and counter semantics
//! with four frameworks. Size alone is a poor proxy for cost, so every program
//! is executed here in a Mollusk VM and charged for the compute units it
//! actually consumes.
//!
//! Usage:
//!
//! ```text
//! framework_verifier --so <PATH> --program-id <BASE58> --case <hello|counter>
//!                    [--hello-data <HEX>] [--expect-log <TEXT>]
//!                    [--initialize-data <HEX>] [--increment-data <HEX>]
//!                    [--initialize-takes-bump]
//!                    [--account-size <N>] [--account-discriminator <HEX>]
//!                    [--bump-offset <N>] [--count-offset <N>]
//! ```
//!
//! The counter's account layout is described on the command line rather than
//! assumed, because the frameworks disagree: Pina, Pinocchio and Quasar store a
//! one-byte discriminator then the bump then the count, while Anchor prefixes an
//! eight-byte discriminator and pads its payload for alignment.
//!
//! Output is one JSON object on stdout. A non-zero exit means the program
//! failed functionally, which the harness treats as a measurement error rather
//! than a result: a fast number from a program that errors out is meaningless.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::program::loader_keys::LOADER_V3;
use mollusk_svm::result::InstructionResult;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_svm_log_collector::LogCollector;

/// Seed byte string the counter PDA is derived from. It must match the seed
/// declared in every counter fixture, because the verifier derives the PDA the
/// program will be handed.
const COUNTER_SEED: &[u8] = b"counter";

/// Fixed authority, so every framework is charged for the same PDA derivation
/// instead of a different bump search.
const AUTHORITY_SEED: [u8; 32] = [1; 32];

fn main() {
	let arguments = Arguments::parse();

	let elf = fs::read(&arguments.so)
		.unwrap_or_else(|error| fail(&format!("read {}: {error}", arguments.so)));
	let program_id: Pubkey = arguments
		.program_id
		.parse()
		.unwrap_or_else(|error| fail(&format!("bad program id: {error}")));

	let mollusk = load(&elf, &program_id);
	let mut steps = Vec::new();

	match arguments.case.as_str() {
		"hello" => {
			let result = process(
				&mollusk,
				&hello_instruction(&program_id, &arguments.hello_data),
				&[(
					Pubkey::new_from_array(AUTHORITY_SEED),
					Account {
						lamports: 1_000_000,
						data: vec![],
						owner: solana_sdk_ids::system_program::id(),
						executable: false,
						rent_epoch: 0,
					},
				)],
			);
			steps.push(step(
				"hello",
				&result,
				&logs(&mollusk),
				&arguments.expect_log,
			));
			require_ok(&result, "hello");
		}
		"counter" => {
			let (counter, bump) =
				Pubkey::find_program_address(&[COUNTER_SEED, AUTHORITY_SEED.as_ref()], &program_id);
			let authority = Pubkey::new_from_array(AUTHORITY_SEED);

			// `initialize` takes the bump as its only argument after the
			// discriminator, so a framework can pass it without re-deriving.
			// Frameworks that derive their own bump from the declared seeds
			// (Quasar, Anchor) read no bump from the instruction.
			let mut initialize_data = arguments.initialize_data.clone();
			if arguments.initialize_takes_bump {
				initialize_data.push(bump);
			}

			let initialize_result = process(
				&mollusk,
				&Instruction::new_with_bytes(
					program_id,
					&initialize_data,
					vec![
						AccountMeta::new(authority, true),
						// The counter is a PDA: the program signs for it through
						// `invoke_signed`, so it is not a transaction signer.
						AccountMeta::new(counter, false),
						AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
					],
				),
				&[
					(
						authority,
						Account {
							lamports: 1_000_000_000,
							data: vec![],
							owner: solana_sdk_ids::system_program::id(),
							executable: false,
							rent_epoch: 0,
						},
					),
					(counter, Account::default()),
					keyed_account_for_system_program(),
				],
			);
			// `initialize` is proven by the account it leaves behind; the
			// expected-log check belongs to the single-instruction case.
			steps.push(step("initialize", &initialize_result, &logs(&mollusk), ""));
			require_ok(&initialize_result, "initialize");

			let stored = |key: &Pubkey| {
				initialize_result
					.resulting_accounts
					.iter()
					.find(|(candidate, _)| candidate == key)
					.map(|(_, account)| account.clone())
					.unwrap_or_else(|| fail(&format!("{key} missing after initialize")))
			};
			let counter_account = stored(&counter);
			let authority_account = stored(&authority);
			verify_counter_state(&counter_account, &arguments, bump);

			// `increment` re-derives the PDA from the stored bump, so it runs
			// against a fresh VM holding the post-initialize accounts.
			let increment_mollusk = load(&elf, &program_id);
			let increment_result = process(
				&increment_mollusk,
				&Instruction::new_with_bytes(
					program_id,
					&arguments.increment_data,
					vec![
						AccountMeta::new_readonly(authority, true),
						AccountMeta::new(counter, false),
					],
				),
				&[
					(authority, authority_account),
					(counter, counter_account.clone()),
				],
			);
			steps.push(step(
				"increment",
				&increment_result,
				&logs(&increment_mollusk),
				"",
			));
			require_ok(&increment_result, "increment");
			verify_counter_value(&increment_result, &counter, &arguments);
		}
		other => fail(&format!("unknown case {other:?}")),
	}

	for step in &steps {
		if !step.ok {
			fail(&format!("{} did not complete successfully", step.name));
		}
	}
	println!("{}", render(&elf, &arguments, &steps));
}

/// Command line for one measurement run.
struct Arguments {
	so: String,
	program_id: String,
	case: String,
	hello_data: Vec<u8>,
	initialize_data: Vec<u8>,
	increment_data: Vec<u8>,
	initialize_takes_bump: bool,
	expect_log: String,
	account_size: usize,
	account_discriminator: Vec<u8>,
	bump_offset: usize,
	count_offset: usize,
}

impl Arguments {
	fn parse() -> Self {
		let mut arguments = Arguments {
			so: String::new(),
			program_id: String::new(),
			case: "hello".to_owned(),
			hello_data: Vec::new(),
			initialize_data: Vec::new(),
			increment_data: Vec::new(),
			initialize_takes_bump: false,
			expect_log: String::new(),
			account_size: 0,
			account_discriminator: Vec::new(),
			bump_offset: 0,
			count_offset: 0,
		};

		let mut argv = std::env::args().skip(1);
		while let Some(flag) = argv.next() {
			let mut value = || {
				argv.next()
					.unwrap_or_else(|| fail(&format!("{flag} needs a value")))
			};
			match flag.as_str() {
				"--so" => arguments.so = value(),
				"--program-id" => arguments.program_id = value(),
				"--case" => arguments.case = value(),
				"--hello-data" => arguments.hello_data = decode_hex(&value()),
				"--initialize-data" => arguments.initialize_data = decode_hex(&value()),
				"--increment-data" => arguments.increment_data = decode_hex(&value()),
				"--initialize-takes-bump" => arguments.initialize_takes_bump = true,
				"--expect-log" => arguments.expect_log = value(),
				"--account-size" => arguments.account_size = parse_usize(&value(), flag.as_str()),
				"--account-discriminator" => arguments.account_discriminator = decode_hex(&value()),
				"--bump-offset" => arguments.bump_offset = parse_usize(&value(), flag.as_str()),
				"--count-offset" => arguments.count_offset = parse_usize(&value(), flag.as_str()),
				other => fail(&format!("unknown flag {other:?}")),
			}
		}

		if arguments.so.is_empty() || arguments.program_id.is_empty() {
			fail("--so and --program-id are required");
		}
		if arguments.case == "counter" && arguments.account_size == 0 {
			fail("the counter case needs --account-size and the account layout offsets");
		}
		arguments
	}
}

/// One measured instruction.
struct Step {
	name: &'static str,
	compute_units: u64,
	ok: bool,
}

fn step(
	name: &'static str,
	result: &InstructionResult,
	program_logs: &[String],
	expect_log: &str,
) -> Step {
	let mut ok = result.program_result.is_ok();
	if !ok {
		for line in program_logs {
			eprintln!("{line}");
		}
	}
	// A program that returns success without doing its job would otherwise be
	// measured as a fast, valid result.
	if ok && !expect_log.is_empty() {
		let found = program_logs.iter().any(|line| line.contains(expect_log));
		if !found {
			ok = false;
			eprintln!("framework_verifier: {name} did not log {expect_log:?}; logs:");
			for line in program_logs {
				eprintln!("  {line}");
			}
		}
	}
	Step {
		name,
		compute_units: result.compute_units_consumed,
		ok,
	}
}

fn render(elf: &[u8], arguments: &Arguments, steps: &[Step]) -> String {
	let instructions = steps
		.iter()
		.map(|step| {
			format!(
				"{{\"name\":\"{}\",\"compute_units\":{},\"ok\":{}}}",
				step.name, step.compute_units, step.ok
			)
		})
		.collect::<Vec<_>>()
		.join(",");

	format!(
		"{{\"case\":\"{}\",\"program_id\":\"{}\",\"bytes\":{},\"instructions\":[{}]}}",
		arguments.case,
		arguments.program_id,
		elf.len(),
		instructions
	)
}

fn load(elf: &[u8], program_id: &Pubkey) -> Mollusk {
	let mut mollusk = Mollusk::default();
	mollusk.add_program_with_loader_and_elf(program_id, &LOADER_V3, elf);
	mollusk.logger = Some(Rc::new(RefCell::new(LogCollector::default())));
	mollusk
}

fn process(
	mollusk: &Mollusk,
	instruction: &Instruction,
	accounts: &[(Pubkey, Account)],
) -> InstructionResult {
	mollusk.process_instruction(instruction, accounts)
}

fn logs(mollusk: &Mollusk) -> Vec<String> {
	mollusk
		.logger
		.as_ref()
		.map(|logger| logger.borrow().get_recorded_content().to_vec())
		.unwrap_or_default()
}

/// The addressed user is the first instruction account in every framework.
fn hello_instruction(program_id: &Pubkey, data: &[u8]) -> Instruction {
	Instruction::new_with_bytes(
		*program_id,
		data,
		vec![AccountMeta::new_readonly(
			Pubkey::new_from_array(AUTHORITY_SEED),
			true,
		)],
	)
}

/// Asserts `initialize` left the account the layout describes: the expected
/// discriminator, the bump the program was given, and a zeroed count.
fn verify_counter_state(account: &Account, arguments: &Arguments, bump: u8) {
	let expected = format!(
		"discriminator {}, size {}, bump at {}, count at {}",
		to_hex(&arguments.account_discriminator),
		arguments.account_size,
		arguments.bump_offset,
		arguments.count_offset
	);
	if account.data.len() != arguments.account_size {
		fail(&format!(
			"counter account is {} bytes, expected {expected}",
			account.data.len()
		));
	}
	if !account.data.starts_with(&arguments.account_discriminator) {
		fail(&format!(
			"counter account is {:02x?}, expected {expected}",
			account.data
		));
	}
	if account.data[arguments.bump_offset] != bump {
		fail(&format!(
			"counter account stores bump {:02x}, expected {bump:02x}; {expected}",
			account.data[arguments.bump_offset]
		));
	}
	if stored_count(account, arguments) != 0 {
		fail(&format!(
			"counter is not zeroed after initialize; {expected}"
		));
	}
}

/// Asserts `increment` stored exactly one on top of the initialized count.
fn verify_counter_value(result: &InstructionResult, counter: &Pubkey, arguments: &Arguments) {
	let Some((_, account)) = result
		.resulting_accounts
		.iter()
		.find(|(candidate, _)| candidate == counter)
	else {
		fail("counter account missing after increment");
	};
	let stored = stored_count(account, arguments);
	if stored != 1 {
		fail(&format!("counter is {stored} after increment, expected 1"));
	}
}

/// Reads the little-endian count from wherever the layout puts it.
fn stored_count(account: &Account, arguments: &Arguments) -> u64 {
	let end = arguments.count_offset + 8;
	let bytes: [u8; 8] = account
		.data
		.get(arguments.count_offset..end)
		.unwrap_or_else(|| fail("counter account is too short to hold its count field"))
		.try_into()
		.expect("slice of eight bytes");
	u64::from_le_bytes(bytes)
}

fn to_hex(bytes: &[u8]) -> String {
	bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_ok(result: &InstructionResult, name: &str) {
	if result.program_result.is_err() {
		fail(&format!("{name} failed: {:?}", result.program_result));
	}
}

fn parse_usize(input: &str, flag: &str) -> usize {
	input
		.parse()
		.unwrap_or_else(|error| fail(&format!("{flag} needs a number: {error}")))
}

fn decode_hex(input: &str) -> Vec<u8> {
	let digits: Vec<u8> = input
		.bytes()
		.filter(|byte| !byte.is_ascii_whitespace())
		.collect();
	if digits.len() % 2 != 0 {
		fail(&format!("hex string has an odd length: {input:?}"));
	}
	digits
		.chunks(2)
		.map(|pair| {
			let text = std::str::from_utf8(pair).unwrap_or_else(|error| fail(&format!("{error}")));
			u8::from_str_radix(text, 16).unwrap_or_else(|error| fail(&format!("{error}")))
		})
		.collect()
}

fn fail(message: &str) -> ! {
	eprintln!("framework_verifier: {message}");
	std::process::exit(1);
}
