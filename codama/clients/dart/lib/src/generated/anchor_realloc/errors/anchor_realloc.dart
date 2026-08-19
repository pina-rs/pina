// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AnchorRealloc program.

const int anchorReallocErrorAccountReallocExceedsLimit = 0xbc8; // 3016

const int anchorReallocErrorAccountDuplicateReallocs = 0xbc9; // 3017

const int anchorReallocErrorAccountDataTooSmall = 0xbca; // 3018

const int anchorReallocErrorAuthorityMismatch = 0xbcb; // 3019

/// Map of error codes to human-readable messages.
const Map<int, String> _anchorReallocErrorMessages = {
  anchorReallocErrorAccountReallocExceedsLimit: '',
  anchorReallocErrorAccountDuplicateReallocs: '',
  anchorReallocErrorAccountDataTooSmall: '',
  anchorReallocErrorAuthorityMismatch: '',
};

/// Get the error message for a AnchorRealloc program error code.
String? getAnchorReallocErrorMessage(int code) {
  return _anchorReallocErrorMessages[code];
}

/// Check if an error code belongs to the AnchorRealloc program.
bool isAnchorReallocError(int code) {
  return _anchorReallocErrorMessages.containsKey(code);
}
