// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the FloatAccountsProgram program.

const int floatAccountsProgramErrorAuthorityMismatch = 0x0; // 0

/// Map of error codes to human-readable messages.
const Map<int, String> _floatAccountsProgramErrorMessages = {
    floatAccountsProgramErrorAuthorityMismatch: '',
};

/// Get the error message for a FloatAccountsProgram program error code.
String? getFloatAccountsProgramErrorMessage(int code) {
  return _floatAccountsProgramErrorMessages[code];
}

/// Check if an error code belongs to the FloatAccountsProgram program.
bool isFloatAccountsProgramError(int code) {
  return _floatAccountsProgramErrorMessages.containsKey(code);
}
