// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the RoleRegistryProgram program.

const int roleRegistryProgramErrorInvalidPermissions = 0x0; // 0

const int roleRegistryProgramErrorRoleAlreadyExists = 0x1; // 1

const int roleRegistryProgramErrorRoleInactive = 0x2; // 2

/// Map of error codes to human-readable messages.
const Map<int, String> _roleRegistryProgramErrorMessages = {
  roleRegistryProgramErrorInvalidPermissions: '',
  roleRegistryProgramErrorRoleAlreadyExists: '',
  roleRegistryProgramErrorRoleInactive: '',
};

/// Get the error message for a RoleRegistryProgram program error code.
String? getRoleRegistryProgramErrorMessage(int code) {
  return _roleRegistryProgramErrorMessages[code];
}

/// Check if an error code belongs to the RoleRegistryProgram program.
bool isRoleRegistryProgramError(int code) {
  return _roleRegistryProgramErrorMessages.containsKey(code);
}
