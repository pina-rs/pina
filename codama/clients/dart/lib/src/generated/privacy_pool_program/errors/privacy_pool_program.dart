// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the PrivacyPoolProgram program.

/// The signer is not the pool authority.
/// Message: "The signer is not the pool authority."
const int privacyPoolProgramErrorInvalidAuthority = 0x0; // 0

/// The commitment is the zero field element.
/// Message: "The commitment is the zero field element."
const int privacyPoolProgramErrorZeroCommitment = 0x1; // 1

/// The note account already exists for this commitment.
/// Message: "The note account already exists for this commitment."
const int privacyPoolProgramErrorNoteAlreadyExists = 0x2; // 2

/// The Merkle tree is full.
/// Message: "The Merkle tree is full."
const int privacyPoolProgramErrorTreeFull = 0x3; // 3

/// The submitted root is not in the recent-root ring.
/// Message: "The submitted root is not in the recent-root ring."
const int privacyPoolProgramErrorUnknownRoot = 0x4; // 4

/// The nullifier has already been spent.
/// Message: "The nullifier has already been spent."
const int privacyPoolProgramErrorNullifierAlreadySpent = 0x5; // 5

/// The nullifier set is full.
/// Message: "The nullifier set is full."
const int privacyPoolProgramErrorNullifierSetFull = 0x6; // 6

/// The Groth16 proof failed verification.
/// Message: "The Groth16 proof failed verification."
const int privacyPoolProgramErrorProofVerificationFailed = 0x7; // 7

/// The verifying key slot has not been installed.
/// Message: "The verifying key slot has not been installed."
const int privacyPoolProgramErrorVerifyingKeyMissing = 0x8; // 8

/// The verifying key slot identifier is invalid.
/// Message: "The verifying key slot identifier is invalid."
const int privacyPoolProgramErrorInvalidVerifyingKeySlot = 0x9; // 9

/// A custodian slot address is zero or duplicated.
/// Message: "A custodian slot address is zero or duplicated."
const int privacyPoolProgramErrorInvalidCustodianSet = 0xa; // 10

/// The signer is not a registered custodian.
/// Message: "The signer is not a registered custodian."
const int privacyPoolProgramErrorNotAcustodian = 0xb; // 11

/// The custodian has already approved this request.
/// Message: "The custodian has already approved this request."
const int privacyPoolProgramErrorAlreadyApproved = 0xc; // 12

/// The disclosure tier is unknown.
/// Message: "The disclosure tier is unknown."
const int privacyPoolProgramErrorInvalidTier = 0xd; // 13

/// The requester is not registered for this tier.
/// Message: "The requester is not registered for this tier."
const int privacyPoolProgramErrorRequesterNotEntitled = 0xe; // 14

/// The requester registry is full.
/// Message: "The requester registry is full."
const int privacyPoolProgramErrorRequesterRegistryFull = 0xf; // 15

/// The target note does not exist.
/// Message: "The target note does not exist."
const int privacyPoolProgramErrorNoteNotFound = 0x10; // 16

/// The request status does not allow this transition.
/// Message: "The request status does not allow this transition."
const int privacyPoolProgramErrorInvalidRequestStatus = 0x11; // 17

/// Consent (tier 0) has not been granted by the note's view key.
/// Message: "Consent (tier 0) has not been granted by the note's view key."
const int privacyPoolProgramErrorConsentRequired = 0x12; // 18

/// The tier-1 challenge window is still open.
/// Message: "The tier-1 challenge window is still open."
const int privacyPoolProgramErrorChallengeWindowOpen = 0x13; // 19

/// Only the note's view key may perform this action.
/// Message: "Only the note's view key may perform this action."
const int privacyPoolProgramErrorNotNoteViewer = 0x14; // 20

/// The disclosure log is full.
/// Message: "The disclosure log is full."
const int privacyPoolProgramErrorLogFull = 0x15; // 21

/// A slice operation went out of bounds.
/// Message: "A slice operation went out of bounds."
const int privacyPoolProgramErrorBufferOverflow = 0x16; // 22

/// A primitive failed its own bounds check.
/// Message: "A primitive failed its own bounds check."
const int privacyPoolProgramErrorArithmeticOverflow = 0x17; // 23

/// Map of error codes to human-readable messages.
const Map<int, String> _privacyPoolProgramErrorMessages = {
  privacyPoolProgramErrorInvalidAuthority:
      'The signer is not the pool authority.',
  privacyPoolProgramErrorZeroCommitment:
      'The commitment is the zero field element.',
  privacyPoolProgramErrorNoteAlreadyExists:
      'The note account already exists for this commitment.',
  privacyPoolProgramErrorTreeFull: 'The Merkle tree is full.',
  privacyPoolProgramErrorUnknownRoot:
      'The submitted root is not in the recent-root ring.',
  privacyPoolProgramErrorNullifierAlreadySpent:
      'The nullifier has already been spent.',
  privacyPoolProgramErrorNullifierSetFull: 'The nullifier set is full.',
  privacyPoolProgramErrorProofVerificationFailed:
      'The Groth16 proof failed verification.',
  privacyPoolProgramErrorVerifyingKeyMissing:
      'The verifying key slot has not been installed.',
  privacyPoolProgramErrorInvalidVerifyingKeySlot:
      'The verifying key slot identifier is invalid.',
  privacyPoolProgramErrorInvalidCustodianSet:
      'A custodian slot address is zero or duplicated.',
  privacyPoolProgramErrorNotAcustodian:
      'The signer is not a registered custodian.',
  privacyPoolProgramErrorAlreadyApproved:
      'The custodian has already approved this request.',
  privacyPoolProgramErrorInvalidTier: 'The disclosure tier is unknown.',
  privacyPoolProgramErrorRequesterNotEntitled:
      'The requester is not registered for this tier.',
  privacyPoolProgramErrorRequesterRegistryFull:
      'The requester registry is full.',
  privacyPoolProgramErrorNoteNotFound: 'The target note does not exist.',
  privacyPoolProgramErrorInvalidRequestStatus:
      'The request status does not allow this transition.',
  privacyPoolProgramErrorConsentRequired:
      'Consent (tier 0) has not been granted by the note\'s view key.',
  privacyPoolProgramErrorChallengeWindowOpen:
      'The tier-1 challenge window is still open.',
  privacyPoolProgramErrorNotNoteViewer:
      'Only the note\'s view key may perform this action.',
  privacyPoolProgramErrorLogFull: 'The disclosure log is full.',
  privacyPoolProgramErrorBufferOverflow:
      'A slice operation went out of bounds.',
  privacyPoolProgramErrorArithmeticOverflow:
      'A primitive failed its own bounds check.',
};

/// Get the error message for a PrivacyPoolProgram program error code.
String? getPrivacyPoolProgramErrorMessage(int code) {
  return _privacyPoolProgramErrorMessages[code];
}

/// Check if an error code belongs to the PrivacyPoolProgram program.
bool isPrivacyPoolProgramError(int code) {
  return _privacyPoolProgramErrorMessages.containsKey(code);
}
