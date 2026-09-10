// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CompactAccountsProgram program.

const int compactAccountsProgramErrorCapacityExceeded = 0x1b58; // 7000

const int compactAccountsProgramErrorIndexOutOfBounds = 0x1b59; // 7001

const int compactAccountsProgramErrorAuthorityMismatch = 0x1b5a; // 7002

/// Map of error codes to human-readable messages.
const Map<int, String> _compactAccountsProgramErrorMessages = {
    compactAccountsProgramErrorCapacityExceeded: '',
    compactAccountsProgramErrorIndexOutOfBounds: '',
    compactAccountsProgramErrorAuthorityMismatch: '',
};

/// Get the error message for a CompactAccountsProgram program error code.
String? getCompactAccountsProgramErrorMessage(int code) {
  return _compactAccountsProgramErrorMessages[code];
}

/// Check if an error code belongs to the CompactAccountsProgram program.
bool isCompactAccountsProgramError(int code) {
  return _compactAccountsProgramErrorMessages.containsKey(code);
}
