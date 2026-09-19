use pina::*;
const _: () = ::pina::ALLOC_FEATURE_REQUIRED_FOR_HEAP_ENTRYPOINT;
/// Program entrypoint.
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    ::pinocchio::entrypoint::process_entrypoint::<
        { ::pina::pinocchio::MAX_TX_ACCOUNTS },
    >(input, process_instruction)
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
