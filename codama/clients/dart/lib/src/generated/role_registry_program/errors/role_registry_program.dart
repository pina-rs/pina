// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the RoleRegistryProgram program.

/// The requested permission bits are empty or outside the supported set.
/// Message: "The requested permission bits are empty or outside the supported set."
const int roleRegistryProgramErrorInvalidPermissions = 0x0; // 0

/// A role with this address is already registered.
/// Message: "A role with this address is already registered."
const int roleRegistryProgramErrorRoleAlreadyExists = 0x1; // 1

/// The role exists but was deactivated, so it grants nothing.
/// Message: "The role exists but was deactivated, so it grants nothing."
const int roleRegistryProgramErrorRoleInactive = 0x2; // 2

/// The proposed admin is the zero address, which can never sign.
/// Message: "The proposed admin is the zero address, which can never sign."
const int roleRegistryProgramErrorZeroAddressAdmin = 0x3; // 3

/// Map of error codes to human-readable messages.
const Map<int, String> _roleRegistryProgramErrorMessages = {
  roleRegistryProgramErrorInvalidPermissions:
      'The requested permission bits are empty or outside the supported set.',
  roleRegistryProgramErrorRoleAlreadyExists:
      'A role with this address is already registered.',
  roleRegistryProgramErrorRoleInactive:
      'The role exists but was deactivated, so it grants nothing.',
  roleRegistryProgramErrorZeroAddressAdmin:
      'The proposed admin is the zero address, which can never sign.',
};

/// Get the error message for a RoleRegistryProgram program error code.
String? getRoleRegistryProgramErrorMessage(int code) {
  return _roleRegistryProgramErrorMessages[code];
}

/// Check if an error code belongs to the RoleRegistryProgram program.
bool isRoleRegistryProgramError(int code) {
  return _roleRegistryProgramErrorMessages.containsKey(code);
}
