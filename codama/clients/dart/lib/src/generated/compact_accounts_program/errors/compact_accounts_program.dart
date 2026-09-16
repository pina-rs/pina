// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CompactAccountsProgram program.

/// The write would exceed the collection's declared capacity.
/// Message: "The write would exceed the collection's declared capacity."
const int compactAccountsProgramErrorCapacityExceeded = 0x1b58; // 7000

/// The requested index is past the end of the active collection.
/// Message: "The requested index is past the end of the active collection."
const int compactAccountsProgramErrorIndexOutOfBounds = 0x1b59; // 7001

/// The signer is not the authority recorded on the account.
/// Message: "The signer is not the authority recorded on the account."
const int compactAccountsProgramErrorAuthorityMismatch = 0x1b5a; // 7002

/// Map of error codes to human-readable messages.
const Map<int, String> _compactAccountsProgramErrorMessages = {
  compactAccountsProgramErrorCapacityExceeded:
      'The write would exceed the collection\'s declared capacity.',
  compactAccountsProgramErrorIndexOutOfBounds:
      'The requested index is past the end of the active collection.',
  compactAccountsProgramErrorAuthorityMismatch:
      'The signer is not the authority recorded on the account.',
};

/// Get the error message for a CompactAccountsProgram program error code.
String? getCompactAccountsProgramErrorMessage(int code) {
  return _compactAccountsProgramErrorMessages[code];
}

/// Check if an error code belongs to the CompactAccountsProgram program.
bool isCompactAccountsProgramError(int code) {
  return _compactAccountsProgramErrorMessages.containsKey(code);
}
