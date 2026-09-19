use pina::*;

nostd_entrypoint!(process_instruction);

fn process_instruction(
	_program_id: &Address,
	_accounts: &mut [AccountView],
	_data: &[u8],
) -> ProgramResult {
	Ok(())
}
