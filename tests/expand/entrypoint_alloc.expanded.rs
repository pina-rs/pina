use pina::*;
/// Program entrypoint.
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    ::pinocchio::entrypoint::process_entrypoint::<
        { ::pina::pinocchio::MAX_TX_ACCOUNTS },
    >(input, process_instruction)
}
/// A default allocator for when the program is compiled on a target different
/// than `"solana"`.
///
/// This links the `std` library, which will set up a default global allocator.
mod __private_alloc {
    extern crate std as __std;
}
/// A panic handler for when the program is compiled on a target different than
/// `"solana"`.
///
/// This links the `std` library, which will set up a default panic handler.
mod __private_panic_handler {
    extern crate std as __std;
}
fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    _data: &[u8],
) -> ProgramResult {
    Ok(())
}
