// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AccountReallocProgram program.

const int accountReallocProgramErrorAccountReallocExceedsLimit = 0xbc8; // 3016

const int accountReallocProgramErrorAccountDuplicateReallocs = 0xbc9; // 3017

const int accountReallocProgramErrorAccountDataTooSmall = 0xbca; // 3018

const int accountReallocProgramErrorAuthorityMismatch = 0xbcb; // 3019

/// Map of error codes to human-readable messages.
const Map<int, String> _accountReallocProgramErrorMessages = {
  accountReallocProgramErrorAccountReallocExceedsLimit: '',
  accountReallocProgramErrorAccountDuplicateReallocs: '',
  accountReallocProgramErrorAccountDataTooSmall: '',
  accountReallocProgramErrorAuthorityMismatch: '',
};

/// Get the error message for a AccountReallocProgram program error code.
String? getAccountReallocProgramErrorMessage(int code) {
  return _accountReallocProgramErrorMessages[code];
}

/// Check if an error code belongs to the AccountReallocProgram program.
bool isAccountReallocProgramError(int code) {
  return _accountReallocProgramErrorMessages.containsKey(code);
}
