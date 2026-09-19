use pina::*;
/// Program entrypoint.
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    ::pinocchio::entrypoint::process_entrypoint::<
        { ::pina::pinocchio::MAX_TX_ACCOUNTS },
    >(input, process_instruction)
}
/// Allocates memory for the given type `T` at the specified offset in the heap
/// reserved address space.
///
/// # Safety
///
/// It is the caller's responsibility to ensure that the offset does not overlap
/// with previous allocations and that type `T` can hold the bit-pattern `0` as
/// a valid value.
///
/// For types that cannot hold the bit-pattern `0` as a valid value, use
/// [`core::mem::MaybeUninit<T>`] to allocate memory for the type and initialize
/// it later.
#[inline(always)]
pub unsafe fn allocate_unchecked<T: Sized>(offset: usize) -> &'static mut T {
    unsafe { &mut *(calculate_offset::<T>(offset) as *mut T) }
}
#[inline(always)]
const fn calculate_offset<T: Sized>(offset: usize) -> usize {
    let start = ::pinocchio::entrypoint::HEAP_START_ADDRESS as usize + offset;
    let end = start + core::mem::size_of::<T>();
    if !(end
        <= ::pinocchio::entrypoint::HEAP_START_ADDRESS as usize
            + ::pinocchio::entrypoint::MAX_HEAP_LENGTH as usize)
    {
        {
            ::core::panicking::panic_fmt(format_args!("allocation exceeds heap size"));
        }
    }
    if !(start % core::mem::align_of::<T>() == 0) {
        {
            ::core::panicking::panic_fmt(format_args!("offset is not aligned"));
        }
    }
    start
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
