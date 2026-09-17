// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the EscrowProgram program.

/// The token accounts do not match the offer's recorded mint and maker.
/// Message: "The token accounts do not match the offer's recorded mint and maker."
const int escrowProgramErrorOfferKeyMismatch = 0x0; // 0

/// A supplied token account is not the one the offer references.
/// Message: "A supplied token account is not the one the offer references."
const int escrowProgramErrorTokenAccountMismatch = 0x1; // 1

/// Map of error codes to human-readable messages.
const Map<int, String> _escrowProgramErrorMessages = {
  escrowProgramErrorOfferKeyMismatch:
      'The token accounts do not match the offer\'s recorded mint and maker.',
  escrowProgramErrorTokenAccountMismatch:
      'A supplied token account is not the one the offer references.',
};

/// Get the error message for a EscrowProgram program error code.
String? getEscrowProgramErrorMessage(int code) {
  return _escrowProgramErrorMessages[code];
}

/// Check if an error code belongs to the EscrowProgram program.
bool isEscrowProgramError(int code) {
  return _escrowProgramErrorMessages.containsKey(code);
}
