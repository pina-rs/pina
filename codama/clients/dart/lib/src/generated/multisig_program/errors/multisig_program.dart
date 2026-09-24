// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the MultisigProgram program.

/// The signer is not a member of the multisig.
/// Message: "The signer is not a member of the multisig."
const int multisigProgramErrorNotAmember = 0x0; // 0

/// The member lacks the permission the instruction requires.
/// Message: "The member lacks the permission the instruction requires."
const int multisigProgramErrorUnauthorized = 0x1; // 1

/// The threshold is zero or exceeds the number of voting members.
/// Message: "The threshold is zero or exceeds the number of voting members."
const int multisigProgramErrorInvalidThreshold = 0x2; // 2

/// The member list exceeds [`MAX_MEMBERS`].
/// Message: "The member list exceeds [`MAX_MEMBERS`]."
const int multisigProgramErrorTooManyMembers = 0x3; // 3

/// The member list contains a duplicate or unsorted key.
/// Message: "The member list contains a duplicate or unsorted key."
const int multisigProgramErrorDuplicateMember = 0x4; // 4

/// A member carries a permission bit outside the defined set.
/// Message: "A member carries a permission bit outside the defined set."
const int multisigProgramErrorUnknownPermission = 0x5; // 5

/// The multisig needs at least one member with each core permission.
/// Message: "The multisig needs at least one member with each core permission."
const int multisigProgramErrorInvalidConfiguration = 0x6; // 6

/// The timelock exceeds [`MAX_TIME_LOCK`].
/// Message: "The timelock exceeds [`MAX_TIME_LOCK`]."
const int multisigProgramErrorTimeLockExceedsMaxAllowed = 0x7; // 7

/// The proposal is not in the status this instruction requires.
/// Message: "The proposal is not in the status this instruction requires."
const int multisigProgramErrorInvalidProposalStatus = 0x8; // 8

/// The proposal predates the last consensus change and is stale.
/// Message: "The proposal predates the last consensus change and is stale."
const int multisigProgramErrorStaleProposal = 0x9; // 9

/// The member has already cast this vote.
/// Message: "The member has already cast this vote."
const int multisigProgramErrorAlreadyVoted = 0xa; // 10

/// The member has no approval to revoke.
/// Message: "The member has no approval to revoke."
const int multisigProgramErrorHasNotApproved = 0xb; // 11

/// The timelock has not elapsed since approval.
/// Message: "The timelock has not elapsed since approval."
const int multisigProgramErrorTimeLockNotReleased = 0xc; // 12

/// The encoded vault message is malformed or exceeds a capacity.
/// Message: "The encoded vault message is malformed or exceeds a capacity."
const int multisigProgramErrorInvalidMessage = 0xd; // 13

/// The remaining accounts do not match the message account keys.
/// Message: "The remaining accounts do not match the message account keys."
const int multisigProgramErrorInvalidNumberOfAccounts = 0xe; // 14

/// An account does not match the key, signer, or writability the message
/// declares.
/// Message: "An account does not match the key, signer, or writability the message"
const int multisigProgramErrorInvalidAccount = 0xf; // 15

/// A message instruction wants to write a program-owned account the
/// multisig must protect.
/// Message: "A message instruction wants to write a program-owned account the"
const int multisigProgramErrorProtectedAccount = 0x10; // 16

/// The encoded config action stream is malformed or exceeds a capacity.
/// Message: "The encoded config action stream is malformed or exceeds a capacity."
const int multisigProgramErrorInvalidActions = 0x11; // 17

/// A config action references a spending limit account that is missing.
/// Message: "A config action references a spending limit account that is missing."
const int multisigProgramErrorMissingAccount = 0x12; // 18

/// Governed config transactions require an autonomous multisig.
/// Message: "Governed config transactions require an autonomous multisig."
const int multisigProgramErrorNotSupportedForControlled = 0x13; // 19

/// The config-authority path requires a controlled multisig.
/// Message: "The config-authority path requires a controlled multisig."
const int multisigProgramErrorNotSupportedForAutonomous = 0x14; // 20

/// The configured creation fee is positive but the treasury account is
/// missing.
/// Message: "The configured creation fee is positive but the treasury account is"
const int multisigProgramErrorMissingTreasury = 0x15; // 21

/// The spending limit is exhausted for this period.
/// Message: "The spending limit is exhausted for this period."
const int multisigProgramErrorSpendingLimitExceeded = 0x16; // 22

/// The destination is not on the spending limit's allow-list.
/// Message: "The destination is not on the spending limit's allow-list."
const int multisigProgramErrorInvalidDestination = 0x17; // 23

/// The mint does not match the spending limit.
/// Message: "The mint does not match the spending limit."
const int multisigProgramErrorInvalidMint = 0x18; // 24

/// The decimals do not match the mint (SOL always has nine).
/// Message: "The decimals do not match the mint (SOL always has nine)."
const int multisigProgramErrorDecimalsMismatch = 0x19; // 25

/// The reset period is not one of the defined variants.
/// Message: "The reset period is not one of the defined variants."
const int multisigProgramErrorInvalidPeriod = 0x1a; // 26

/// The legacy account is not a multisig this program can import.
/// Message: "The legacy account is not a multisig this program can import."
const int multisigProgramErrorInvalidLegacyMultisig = 0x1b; // 27

/// The program config authority does not match the signer.
/// Message: "The program config authority does not match the signer."
const int multisigProgramErrorInvalidConfigAuthority = 0x1c; // 28

/// The proposal kind does not match the instruction.
/// Message: "The proposal kind does not match the instruction."
const int multisigProgramErrorInvalidProposalKind = 0x1d; // 29

/// A spending limit action needs its rent payer and system program.
/// Message: "A spending limit action needs its rent payer and system program."
const int multisigProgramErrorMissingRentPayer = 0x1e; // 30

/// The proposal's recorded lifetime has elapsed.
/// Message: "The proposal's recorded lifetime has elapsed."
const int multisigProgramErrorProposalExpired = 0x1f; // 31

/// A spending-limit action moves vault funds and requires a governed
/// proposal, not the instant config-authority path.
/// Message: "A spending-limit action moves vault funds and requires a governed"
const int multisigProgramErrorSpendingLimitRequiresProposal = 0x20; // 32

/// Map of error codes to human-readable messages.
const Map<int, String> _multisigProgramErrorMessages = {
  multisigProgramErrorNotAmember: 'The signer is not a member of the multisig.',
  multisigProgramErrorUnauthorized:
      'The member lacks the permission the instruction requires.',
  multisigProgramErrorInvalidThreshold:
      'The threshold is zero or exceeds the number of voting members.',
  multisigProgramErrorTooManyMembers:
      'The member list exceeds [`MAX_MEMBERS`].',
  multisigProgramErrorDuplicateMember:
      'The member list contains a duplicate or unsorted key.',
  multisigProgramErrorUnknownPermission:
      'A member carries a permission bit outside the defined set.',
  multisigProgramErrorInvalidConfiguration:
      'The multisig needs at least one member with each core permission.',
  multisigProgramErrorTimeLockExceedsMaxAllowed:
      'The timelock exceeds [`MAX_TIME_LOCK`].',
  multisigProgramErrorInvalidProposalStatus:
      'The proposal is not in the status this instruction requires.',
  multisigProgramErrorStaleProposal:
      'The proposal predates the last consensus change and is stale.',
  multisigProgramErrorAlreadyVoted: 'The member has already cast this vote.',
  multisigProgramErrorHasNotApproved: 'The member has no approval to revoke.',
  multisigProgramErrorTimeLockNotReleased:
      'The timelock has not elapsed since approval.',
  multisigProgramErrorInvalidMessage:
      'The encoded vault message is malformed or exceeds a capacity.',
  multisigProgramErrorInvalidNumberOfAccounts:
      'The remaining accounts do not match the message account keys.',
  multisigProgramErrorInvalidAccount:
      'An account does not match the key, signer, or writability the message',
  multisigProgramErrorProtectedAccount:
      'A message instruction wants to write a program-owned account the',
  multisigProgramErrorInvalidActions:
      'The encoded config action stream is malformed or exceeds a capacity.',
  multisigProgramErrorMissingAccount:
      'A config action references a spending limit account that is missing.',
  multisigProgramErrorNotSupportedForControlled:
      'Governed config transactions require an autonomous multisig.',
  multisigProgramErrorNotSupportedForAutonomous:
      'The config-authority path requires a controlled multisig.',
  multisigProgramErrorMissingTreasury:
      'The configured creation fee is positive but the treasury account is',
  multisigProgramErrorSpendingLimitExceeded:
      'The spending limit is exhausted for this period.',
  multisigProgramErrorInvalidDestination:
      'The destination is not on the spending limit\'s allow-list.',
  multisigProgramErrorInvalidMint:
      'The mint does not match the spending limit.',
  multisigProgramErrorDecimalsMismatch:
      'The decimals do not match the mint (SOL always has nine).',
  multisigProgramErrorInvalidPeriod:
      'The reset period is not one of the defined variants.',
  multisigProgramErrorInvalidLegacyMultisig:
      'The legacy account is not a multisig this program can import.',
  multisigProgramErrorInvalidConfigAuthority:
      'The program config authority does not match the signer.',
  multisigProgramErrorInvalidProposalKind:
      'The proposal kind does not match the instruction.',
  multisigProgramErrorMissingRentPayer:
      'A spending limit action needs its rent payer and system program.',
  multisigProgramErrorProposalExpired:
      'The proposal\'s recorded lifetime has elapsed.',
  multisigProgramErrorSpendingLimitRequiresProposal:
      'A spending-limit action moves vault funds and requires a governed',
};

/// Get the error message for a MultisigProgram program error code.
String? getMultisigProgramErrorMessage(int code) {
  return _multisigProgramErrorMessages[code];
}

/// Check if an error code belongs to the MultisigProgram program.
bool isMultisigProgramError(int code) {
  return _multisigProgramErrorMessages.containsKey(code);
}
