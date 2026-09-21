// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the ValidationProgram program.

/// The lower policy bound must not exceed the upper bound.
/// Message: "The lower policy bound must not exceed the upper bound."
const int validationProgramErrorInvalidPolicyRange = 0x1; // 1

/// An instruction amount is outside the absolute limits of this program.
/// Message: "An instruction amount is outside the absolute limits of this program."
const int validationProgramErrorInvalidAmount = 0x2; // 2

/// A human-readable memo is too short or too long.
/// Message: "A human-readable memo is too short or too long."
const int validationProgramErrorInvalidMemo = 0x3; // 3

/// A check must contain exactly two different approval codes.
/// Message: "A check must contain exactly two different approval codes."
const int validationProgramErrorInvalidApprovals = 0x4; // 4

/// The validated account list violates a relationship between accounts.
/// Message: "The validated account list violates a relationship between accounts."
const int validationProgramErrorInvalidAccounts = 0x5; // 5

/// The amount does not fall inside the bounds stored in the policy account.
/// Message: "The amount does not fall inside the bounds stored in the policy account."
const int validationProgramErrorAmountOutsidePolicy = 0x6; // 6

/// The event would describe an invalid policy check.
/// Message: "The event would describe an invalid policy check."
const int validationProgramErrorInvalidEvent = 0x7; // 7

/// Map of error codes to human-readable messages.
const Map<int, String> _validationProgramErrorMessages = {
  validationProgramErrorInvalidPolicyRange:
      'The lower policy bound must not exceed the upper bound.',
  validationProgramErrorInvalidAmount:
      'An instruction amount is outside the absolute limits of this program.',
  validationProgramErrorInvalidMemo:
      'A human-readable memo is too short or too long.',
  validationProgramErrorInvalidApprovals:
      'A check must contain exactly two different approval codes.',
  validationProgramErrorInvalidAccounts:
      'The validated account list violates a relationship between accounts.',
  validationProgramErrorAmountOutsidePolicy:
      'The amount does not fall inside the bounds stored in the policy account.',
  validationProgramErrorInvalidEvent:
      'The event would describe an invalid policy check.',
};

/// Get the error message for a ValidationProgram program error code.
String? getValidationProgramErrorMessage(int code) {
  return _validationProgramErrorMessages[code];
}

/// Check if an error code belongs to the ValidationProgram program.
bool isValidationProgramError(int code) {
  return _validationProgramErrorMessages.containsKey(code);
}
