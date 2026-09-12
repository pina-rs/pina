// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the DuplicateMutableAccountsProgram program.

const int duplicateMutableAccountsProgramErrorConstraintDuplicateMutableAccount = 0x7f8; // 2040

/// Map of error codes to human-readable messages.
const Map<int, String> _duplicateMutableAccountsProgramErrorMessages = {
    duplicateMutableAccountsProgramErrorConstraintDuplicateMutableAccount: '',
};

/// Get the error message for a DuplicateMutableAccountsProgram program error code.
String? getDuplicateMutableAccountsProgramErrorMessage(int code) {
  return _duplicateMutableAccountsProgramErrorMessages[code];
}

/// Check if an error code belongs to the DuplicateMutableAccountsProgram program.
bool isDuplicateMutableAccountsProgramError(int code) {
  return _duplicateMutableAccountsProgramErrorMessages.containsKey(code);
}
