// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AccountRealloc program.

const int accountReallocErrorAccountReallocExceedsLimit = 0xbc8; // 3016

const int accountReallocErrorAccountDuplicateReallocs = 0xbc9; // 3017

const int accountReallocErrorAccountDataTooSmall = 0xbca; // 3018

const int accountReallocErrorAuthorityMismatch = 0xbcb; // 3019

/// Map of error codes to human-readable messages.
const Map<int, String> _accountReallocErrorMessages = {
  accountReallocErrorAccountReallocExceedsLimit: '',
  accountReallocErrorAccountDuplicateReallocs: '',
  accountReallocErrorAccountDataTooSmall: '',
  accountReallocErrorAuthorityMismatch: '',
};

/// Get the error message for a AccountRealloc program error code.
String? getAccountReallocErrorMessage(int code) {
  return _accountReallocErrorMessages[code];
}

/// Check if an error code belongs to the AccountRealloc program.
bool isAccountReallocError(int code) {
  return _accountReallocErrorMessages.containsKey(code);
}
