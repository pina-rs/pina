// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CompactAccounts program.

const int compactAccountsErrorCapacityExceeded = 0x1b58; // 7000

const int compactAccountsErrorIndexOutOfBounds = 0x1b59; // 7001

const int compactAccountsErrorAuthorityMismatch = 0x1b5a; // 7002

/// Map of error codes to human-readable messages.
const Map<int, String> _compactAccountsErrorMessages = {
  compactAccountsErrorCapacityExceeded: '',
  compactAccountsErrorIndexOutOfBounds: '',
  compactAccountsErrorAuthorityMismatch: '',
};

/// Get the error message for a CompactAccounts program error code.
String? getCompactAccountsErrorMessage(int code) {
  return _compactAccountsErrorMessages[code];
}

/// Check if an error code belongs to the CompactAccounts program.
bool isCompactAccountsError(int code) {
  return _compactAccountsErrorMessages.containsKey(code);
}
