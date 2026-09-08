// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the FloatAccounts program.

const int floatAccountsErrorAuthorityMismatch = 0x0; // 0

/// Map of error codes to human-readable messages.
const Map<int, String> _floatAccountsErrorMessages = {
  floatAccountsErrorAuthorityMismatch: '',
};

/// Get the error message for a FloatAccounts program error code.
String? getFloatAccountsErrorMessage(int code) {
  return _floatAccountsErrorMessages[code];
}

/// Check if an error code belongs to the FloatAccounts program.
bool isFloatAccountsError(int code) {
  return _floatAccountsErrorMessages.containsKey(code);
}
