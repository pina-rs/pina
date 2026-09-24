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

/// The supplied reward index would move rewards backwards.
/// Message: "The supplied reward index would move rewards backwards."
const int stakingRewardsProgramErrorRewardIndexRegressed = 0x5; // 5

/// The position has accrued nothing to release.
/// Message: "The position has accrued nothing to release."
const int stakingRewardsProgramErrorNothingToClaim = 0x6; // 6

/// The reward index would create liabilities no `u64` payout can
/// represent, freezing affected positions at their next checkpoint.
/// Message: "The reward index would create liabilities no `u64` payout can"
const int stakingRewardsProgramErrorRewardIndexExceedsCapacity = 0x7; // 7

/// The reward index would create liabilities beyond the reward vault's
/// balance, making equal entitlements depend on claim order.
/// Message: "The reward index would create liabilities beyond the reward vault's"
const int stakingRewardsProgramErrorRewardIndexExceedsReserves = 0x8; // 8

/// Map of error codes to human-readable messages.
const Map<int, String> _stakingRewardsProgramErrorMessages = {
    stakingRewardsProgramErrorInvalidAmount: 'The amount is zero, or it leaves a position below the pool minimum.',
    stakingRewardsProgramErrorPoolPaused: 'The pool is paused, so deposits and withdrawals are refused.',
    stakingRewardsProgramErrorInsufficientBalance: 'The position holds less than the requested withdrawal.',
    stakingRewardsProgramErrorUnauthorized: 'The signer is not the pool authority this instruction requires.',
    stakingRewardsProgramErrorInvalidPool: 'The supplied account is not the pool this position belongs to.',
    stakingRewardsProgramErrorRewardIndexRegressed: 'The supplied reward index would move rewards backwards.',
    stakingRewardsProgramErrorNothingToClaim: 'The position has accrued nothing to release.',
    stakingRewardsProgramErrorRewardIndexExceedsCapacity: 'The reward index would create liabilities no `u64` payout can',
    stakingRewardsProgramErrorRewardIndexExceedsReserves: 'The reward index would create liabilities beyond the reward vault\'s',
};

/// Get the error message for a StakingRewardsProgram program error code.
String? getStakingRewardsProgramErrorMessage(int code) {
  return _stakingRewardsProgramErrorMessages[code];
}

/// Check if an error code belongs to the StakingRewardsProgram program.
bool isStakingRewardsProgramError(int code) {
  return _stakingRewardsProgramErrorMessages.containsKey(code);
}
