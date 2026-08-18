// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the ProfileProgram program.

/// A bounded string field contained invalid UTF-8.
/// Message: "A bounded string field contained invalid UTF-8."
const int profileProgramErrorInvalidUtf8 = 0x0; // 0

/// The tag list is full (capacity 8).
/// Message: "The tag list is full (capacity 8)."
const int profileProgramErrorTagOverflow = 0x1; // 1

/// The tag index is out of range.
/// Message: "The tag index is out of range."
const int profileProgramErrorTagNotFound = 0x2; // 2

/// Map of error codes to human-readable messages.
const Map<int, String> _profileProgramErrorMessages = {
  profileProgramErrorInvalidUtf8:
      'A bounded string field contained invalid UTF-8.',
  profileProgramErrorTagOverflow: 'The tag list is full (capacity 8).',
  profileProgramErrorTagNotFound: 'The tag index is out of range.',
};

/// Get the error message for a ProfileProgram program error code.
String? getProfileProgramErrorMessage(int code) {
  return _profileProgramErrorMessages[code];
}

/// Check if an error code belongs to the ProfileProgram program.
bool isProfileProgramError(int code) {
  return _profileProgramErrorMessages.containsKey(code);
}
