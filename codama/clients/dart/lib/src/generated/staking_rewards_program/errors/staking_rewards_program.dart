// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the StakingRewardsProgram program.

/// The amount is zero, or it leaves a position below the pool minimum.
/// Message: "The amount is zero, or it leaves a position below the pool minimum."
const int stakingRewardsProgramErrorInvalidAmount = 0x0; // 0

/// The pool is paused, so deposits and withdrawals are refused.
/// Message: "The pool is paused, so deposits and withdrawals are refused."
const int stakingRewardsProgramErrorPoolPaused = 0x1; // 1

/// The position holds less than the requested withdrawal.
/// Message: "The position holds less than the requested withdrawal."
const int stakingRewardsProgramErrorInsufficientBalance = 0x2; // 2

/// The signer is not the pool authority this instruction requires.
/// Message: "The signer is not the pool authority this instruction requires."
const int stakingRewardsProgramErrorUnauthorized = 0x3; // 3

/// The supplied account is not the pool this position belongs to.
/// Message: "The supplied account is not the pool this position belongs to."
const int stakingRewardsProgramErrorInvalidPool = 0x4; // 4

/// Map of error codes to human-readable messages.
const Map<int, String> _stakingRewardsProgramErrorMessages = {
  stakingRewardsProgramErrorInvalidAmount:
      'The amount is zero, or it leaves a position below the pool minimum.',
  stakingRewardsProgramErrorPoolPaused:
      'The pool is paused, so deposits and withdrawals are refused.',
  stakingRewardsProgramErrorInsufficientBalance:
      'The position holds less than the requested withdrawal.',
  stakingRewardsProgramErrorUnauthorized:
      'The signer is not the pool authority this instruction requires.',
  stakingRewardsProgramErrorInvalidPool:
      'The supplied account is not the pool this position belongs to.',
};

/// Get the error message for a StakingRewardsProgram program error code.
String? getStakingRewardsProgramErrorMessage(int code) {
  return _stakingRewardsProgramErrorMessages[code];
}

/// Check if an error code belongs to the StakingRewardsProgram program.
bool isStakingRewardsProgramError(int code) {
  return _stakingRewardsProgramErrorMessages.containsKey(code);
}
