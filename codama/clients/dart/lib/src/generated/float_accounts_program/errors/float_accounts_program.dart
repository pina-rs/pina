// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the FloatAccountsProgram program.

/// The signer is not the authority recorded on the account.
/// Message: "The signer is not the authority recorded on the account."
const int floatAccountsProgramErrorAuthorityMismatch = 0x0; // 0

/// A float payload is NaN or infinite; only finite values are stored.
/// Message: "A float payload is NaN or infinite; only finite values are stored."
const int floatAccountsProgramErrorNonFiniteFloat = 0x1; // 1

/// Map of error codes to human-readable messages.
const Map<int, String> _floatAccountsProgramErrorMessages = {
    floatAccountsProgramErrorAuthorityMismatch: 'The signer is not the authority recorded on the account.',
    floatAccountsProgramErrorNonFiniteFloat: 'A float payload is NaN or infinite; only finite values are stored.',
};

/// Get the error message for a FloatAccountsProgram program error code.
String? getFloatAccountsProgramErrorMessage(int code) {
  return _floatAccountsProgramErrorMessages[code];
}

/// Check if an error code belongs to the FloatAccountsProgram program.
bool isFloatAccountsProgramError(int code) {
  return _floatAccountsProgramErrorMessages.containsKey(code);
}
