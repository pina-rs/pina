// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the TransferSol program.

/// The sender does not have enough lamports for the transfer.
/// Message: "The sender does not have enough lamports for the transfer."
const int transferSolErrorInsufficientFunds = 0x0; // 0

/// Map of error codes to human-readable messages.
const Map<int, String> _transferSolErrorMessages = {
  transferSolErrorInsufficientFunds:
      'The sender does not have enough lamports for the transfer.',
};

/// Get the error message for a TransferSol program error code.
String? getTransferSolErrorMessage(int code) {
  return _transferSolErrorMessages[code];
}

/// Check if an error code belongs to the TransferSol program.
bool isTransferSolError(int code) {
  return _transferSolErrorMessages.containsKey(code);
}
