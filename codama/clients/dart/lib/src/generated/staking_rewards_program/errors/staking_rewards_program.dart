// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the StakingRewardsProgram program.

const int stakingRewardsProgramErrorInvalidAmount = 0x0; // 0

const int stakingRewardsProgramErrorPoolPaused = 0x1; // 1

const int stakingRewardsProgramErrorInsufficientBalance = 0x2; // 2

const int stakingRewardsProgramErrorUnauthorized = 0x3; // 3

const int stakingRewardsProgramErrorInvalidPool = 0x4; // 4

/// Map of error codes to human-readable messages.
const Map<int, String> _stakingRewardsProgramErrorMessages = {
  stakingRewardsProgramErrorInvalidAmount: '',
  stakingRewardsProgramErrorPoolPaused: '',
  stakingRewardsProgramErrorInsufficientBalance: '',
  stakingRewardsProgramErrorUnauthorized: '',
  stakingRewardsProgramErrorInvalidPool: '',
};

/// Get the error message for a StakingRewardsProgram program error code.
String? getStakingRewardsProgramErrorMessage(int code) {
  return _stakingRewardsProgramErrorMessages[code];
}

/// Check if an error code belongs to the StakingRewardsProgram program.
bool isStakingRewardsProgramError(int code) {
  return _stakingRewardsProgramErrorMessages.containsKey(code);
}
