// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the EscrowProgram program.

const int escrowProgramErrorOfferKeyMismatch = 0x0; // 0

const int escrowProgramErrorTokenAccountMismatch = 0x1; // 1

/// Map of error codes to human-readable messages.
const Map<int, String> _escrowProgramErrorMessages = {
  escrowProgramErrorOfferKeyMismatch: '',
  escrowProgramErrorTokenAccountMismatch: '',
};

/// Get the error message for a EscrowProgram program error code.
String? getEscrowProgramErrorMessage(int code) {
  return _escrowProgramErrorMessages[code];
}

/// Check if an error code belongs to the EscrowProgram program.
bool isEscrowProgramError(int code) {
  return _escrowProgramErrorMessages.containsKey(code);
}
