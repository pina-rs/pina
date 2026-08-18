// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AnchorDuplicateMutableAccounts program.

const int anchorDuplicateMutableAccountsErrorConstraintDuplicateMutableAccount =
    0x7f8; // 2040

/// Map of error codes to human-readable messages.
const Map<int, String> _anchorDuplicateMutableAccountsErrorMessages = {
  anchorDuplicateMutableAccountsErrorConstraintDuplicateMutableAccount: '',
};

/// Get the error message for a AnchorDuplicateMutableAccounts program error code.
String? getAnchorDuplicateMutableAccountsErrorMessage(int code) {
  return _anchorDuplicateMutableAccountsErrorMessages[code];
}

/// Check if an error code belongs to the AnchorDuplicateMutableAccounts program.
bool isAnchorDuplicateMutableAccountsError(int code) {
  return _anchorDuplicateMutableAccountsErrorMessages.containsKey(code);
}
