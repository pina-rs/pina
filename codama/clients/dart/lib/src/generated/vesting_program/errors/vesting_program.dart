// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the VestingProgram program.

const int vestingProgramErrorInvalidSchedule = 0x0; // 0

const int vestingProgramErrorClaimTooLarge = 0x1; // 1

const int vestingProgramErrorAlreadyCancelled = 0x2; // 2

/// Map of error codes to human-readable messages.
const Map<int, String> _vestingProgramErrorMessages = {
  vestingProgramErrorInvalidSchedule: '',
  vestingProgramErrorClaimTooLarge: '',
  vestingProgramErrorAlreadyCancelled: '',
};

/// Get the error message for a VestingProgram program error code.
String? getVestingProgramErrorMessage(int code) {
  return _vestingProgramErrorMessages[code];
}

/// Check if an error code belongs to the VestingProgram program.
bool isVestingProgramError(int code) {
  return _vestingProgramErrorMessages.containsKey(code);
}
