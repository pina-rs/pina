use pina::*;

nostd_entrypoint_alloc!(process_instruction);

fn process_instruction(
	_program_id: &Address,
	_accounts: &mut [AccountView],
	_data: &[u8],
) -> ProgramResult {
	Ok(())
}
