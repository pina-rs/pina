// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio_token.rs
// aux-build: pina.rs

#![allow(dead_code, unused_variables)]

extern crate pina;
extern crate pinocchio_token;

use std::cell::UnsafeCell;
use std::sync::Mutex;

use pina::AccountInfoValidation;
use pina::ProgramAccount as PinaProgramAccount;
use pinocchio_token::Address;
use pinocchio_token::Instruction;
use pinocchio_token::InstructionFor;

// A lock-guarded static is immutable in Rust's sense, yet its contents can be
// replaced from instruction data at runtime.
static MUTEX_ID: Mutex<Address> = Mutex::new(Address);

// A raw `UnsafeCell` wrapper is interior-mutable by construction.
struct RawCell {
	value: UnsafeCell<Address>,
}

unsafe impl Sync for RawCell {}

static UNSAFE_CELL_ID: RawCell = RawCell {
	value: UnsafeCell::new(Address),
};

// Interior mutability survives one level of struct wrapping.
struct Guarded {
	inner: Mutex<Address>,
}

static GUARDED_ID: Guarded = Guarded {
	inner: Mutex::new(Address),
};

// Interior-mutability-free statics remain trusted provenance.
static ADDRESS_ID: Address = Address;
static BYTE_ID: [u8; 32] = [42; 32];

type MutexAccount = PinaProgramAccount<Mutex<Address>>;
type UnsafeCellAccount = PinaProgramAccount<RawCell>;
type GuardedAccount = PinaProgramAccount<Guarded>;
type ProgramAccount = PinaProgramAccount<Address>;
type BytesAccount = PinaProgramAccount<[u8; 32]>;

fn process_mutex_static(
	instruction: &InstructionFor<Mutex<Address>>,
	program: &MutexAccount,
) -> Result<(), ()> {
	// The rewritten contents make the later comparison attacker-controlled,
	// so the immutable static must not establish a proof.
	*MUTEX_ID.lock().unwrap() = Address;
	program.assert_program(&MUTEX_ID)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_unsafe_cell_static(
	instruction: &InstructionFor<RawCell>,
	program: &UnsafeCellAccount,
) -> Result<(), ()> {
	program.assert_program(&UNSAFE_CELL_ID)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_guarded_static(
	instruction: &InstructionFor<Guarded>,
	program: &GuardedAccount,
) -> Result<(), ()> {
	program.assert_program(&GUARDED_ID)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_interior_mutable_cpi_target(
	instruction: &InstructionFor<Mutex<Address>>,
) -> Result<(), ()> {
	// An interior-mutable static cannot serve as the dynamic target itself.
	instruction.invoke_with_unverified_program(&MUTEX_ID)
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

// A const of reference type can alias an interior-mutable static, so the const
// would launder the static's runtime-replaceable contents past this check. The
// reference type is not `Freeze`, so the const must not establish a proof.
const MUTEX_ALIAS: &Mutex<Address> = &MUTEX_ID;

fn process_const_alias(
	instruction: &InstructionFor<Mutex<Address>>,
	program: &MutexAccount,
) -> Result<(), ()> {
	*MUTEX_ID.lock().unwrap() = Address;
	program.assert_program(MUTEX_ALIAS)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_plain_statics(instruction: &Instruction, program: &ProgramAccount) -> Result<(), ()> {
	program.assert_program(&ADDRESS_ID)?;
	instruction.invoke_with_unverified_program(program.address())
}

// Interior-mutability-free consts remain trusted provenance.
const ADDRESS_CONST: Address = Address;

fn process_const(instruction: &Instruction, program: &ProgramAccount) -> Result<(), ()> {
	program.assert_program(&ADDRESS_CONST)?;
	instruction.invoke_with_unverified_program(program.address())
}

fn process_copy_static(
	instruction: &InstructionFor<[u8; 32]>,
	program: &BytesAccount,
) -> Result<(), ()> {
	program.assert_addresses(&[BYTE_ID])?;
	instruction.invoke_with_unverified_program(program.address())
}

fn main() {}

// compile-fail
