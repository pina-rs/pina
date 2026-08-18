// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AnchorFloats program.

const int anchorFloatsErrorAuthorityMismatch = 0x0; // 0

/// Map of error codes to human-readable messages.
const Map<int, String> _anchorFloatsErrorMessages = {
  anchorFloatsErrorAuthorityMismatch: '',
};

/// Get the error message for a AnchorFloats program error code.
String? getAnchorFloatsErrorMessage(int code) {
  return _anchorFloatsErrorMessages[code];
}

/// Check if an error code belongs to the AnchorFloats program.
bool isAnchorFloatsError(int code) {
  return _anchorFloatsErrorMessages.containsKey(code);
}
