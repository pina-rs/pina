// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AccountReallocProgram program.

/// The requested growth exceeds the runtime's per-instruction realloc limit.
/// Message: "The requested growth exceeds the runtime's per-instruction realloc limit."
const int accountReallocProgramErrorAccountReallocExceedsLimit = 0xbc8; // 3016

/// The same account was passed to more than one realloc slot.
/// Message: "The same account was passed to more than one realloc slot."
const int accountReallocProgramErrorAccountDuplicateReallocs = 0xbc9; // 3017

/// The account is smaller than the data the instruction writes.
/// Message: "The account is smaller than the data the instruction writes."
const int accountReallocProgramErrorAccountDataTooSmall = 0xbca; // 3018

/// The signer is not the authority recorded on the account.
/// Message: "The signer is not the authority recorded on the account."
const int accountReallocProgramErrorAuthorityMismatch = 0xbcb; // 3019

/// Map of error codes to human-readable messages.
const Map<int, String> _accountReallocProgramErrorMessages = {
  accountReallocProgramErrorAccountReallocExceedsLimit:
      'The requested growth exceeds the runtime\'s per-instruction realloc limit.',
  accountReallocProgramErrorAccountDuplicateReallocs:
      'The same account was passed to more than one realloc slot.',
  accountReallocProgramErrorAccountDataTooSmall:
      'The account is smaller than the data the instruction writes.',
  accountReallocProgramErrorAuthorityMismatch:
      'The signer is not the authority recorded on the account.',
};

/// Get the error message for a AccountReallocProgram program error code.
String? getAccountReallocProgramErrorMessage(int code) {
  return _accountReallocProgramErrorMessages[code];
}

/// Check if an error code belongs to the AccountReallocProgram program.
bool isAccountReallocProgramError(int code) {
  return _accountReallocProgramErrorMessages.containsKey(code);
}
