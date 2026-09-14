// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the PropAmmProgram program.

const int propAmmProgramErrorUnauthorizedUpdateAuthority = 0x0; // 0

const int propAmmProgramErrorUnauthorizedOracleAuthority = 0x1; // 1

/// Map of error codes to human-readable messages.
const Map<int, String> _propAmmProgramErrorMessages = {
  propAmmProgramErrorUnauthorizedUpdateAuthority: '',
  propAmmProgramErrorUnauthorizedOracleAuthority: '',
};

/// Get the error message for a PropAmmProgram program error code.
String? getPropAmmProgramErrorMessage(int code) {
  return _propAmmProgramErrorMessages[code];
}

/// Check if an error code belongs to the PropAmmProgram program.
bool isPropAmmProgramError(int code) {
  return _propAmmProgramErrorMessages.containsKey(code);
}
