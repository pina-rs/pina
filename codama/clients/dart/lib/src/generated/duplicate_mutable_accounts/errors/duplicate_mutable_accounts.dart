// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the DuplicateMutableAccounts program.

const int duplicateMutableAccountsErrorConstraintDuplicateMutableAccount =
    0x7f8; // 2040

/// Map of error codes to human-readable messages.
const Map<int, String> _duplicateMutableAccountsErrorMessages = {
  duplicateMutableAccountsErrorConstraintDuplicateMutableAccount: '',
};

/// Get the error message for a DuplicateMutableAccounts program error code.
String? getDuplicateMutableAccountsErrorMessage(int code) {
  return _duplicateMutableAccountsErrorMessages[code];
}

/// Check if an error code belongs to the DuplicateMutableAccounts program.
bool isDuplicateMutableAccountsError(int code) {
  return _duplicateMutableAccountsErrorMessages.containsKey(code);
}
