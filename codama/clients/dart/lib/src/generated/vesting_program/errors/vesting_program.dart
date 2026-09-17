// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the VestingProgram program.

/// The schedule is malformed: its window is empty, unordered, or fully elapsed.
/// Message: "The schedule is malformed: its window is empty, unordered, or fully elapsed."
const int vestingProgramErrorInvalidSchedule = 0x0; // 0

/// The claim exceeds what has vested so far.
/// Message: "The claim exceeds what has vested so far."
const int vestingProgramErrorClaimTooLarge = 0x1; // 1

/// The vesting account was already cancelled and holds nothing to claim.
/// Message: "The vesting account was already cancelled and holds nothing to claim."
const int vestingProgramErrorAlreadyCancelled = 0x2; // 2

/// Map of error codes to human-readable messages.
const Map<int, String> _vestingProgramErrorMessages = {
    vestingProgramErrorInvalidSchedule: 'The schedule is malformed: its window is empty, unordered, or fully elapsed.',
    vestingProgramErrorClaimTooLarge: 'The claim exceeds what has vested so far.',
    vestingProgramErrorAlreadyCancelled: 'The vesting account was already cancelled and holds nothing to claim.',
};

/// Get the error message for a VestingProgram program error code.
String? getVestingProgramErrorMessage(int code) {
  return _vestingProgramErrorMessages[code];
}

/// Check if an error code belongs to the VestingProgram program.
bool isVestingProgramError(int code) {
  return _vestingProgramErrorMessages.containsKey(code);
}
