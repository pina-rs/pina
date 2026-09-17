// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the TransferSolProgram program.

/// The sender does not have enough lamports for the transfer.
/// Message: "The sender does not have enough lamports for the transfer."
const int transferSolProgramErrorInsufficientFunds = 0x0; // 0

/// Map of error codes to human-readable messages.
const Map<int, String> _transferSolProgramErrorMessages = {
    transferSolProgramErrorInsufficientFunds: 'The sender does not have enough lamports for the transfer.',
};

/// Get the error message for a TransferSolProgram program error code.
String? getTransferSolProgramErrorMessage(int code) {
  return _transferSolProgramErrorMessages[code];
}

/// Check if an error code belongs to the TransferSolProgram program.
bool isTransferSolProgramError(int code) {
  return _transferSolProgramErrorMessages.containsKey(code);
}
