// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CustomErrorsProgram program.

const int customErrorsProgramErrorHello = 0x1770; // 6000

const int customErrorsProgramErrorHelloNoMsg = 0x17eb; // 6123

const int customErrorsProgramErrorHelloNext = 0x17ec; // 6124

const int customErrorsProgramErrorHelloCustom = 0x17ed; // 6125

const int customErrorsProgramErrorValueMismatch = 0x17ee; // 6126

const int customErrorsProgramErrorValueMatch = 0x17ef; // 6127

const int customErrorsProgramErrorValueLess = 0x17f0; // 6128

const int customErrorsProgramErrorValueLessOrEqual = 0x17f1; // 6129

/// Map of error codes to human-readable messages.
const Map<int, String> _customErrorsProgramErrorMessages = {
    customErrorsProgramErrorHello: '',
    customErrorsProgramErrorHelloNoMsg: '',
    customErrorsProgramErrorHelloNext: '',
    customErrorsProgramErrorHelloCustom: '',
    customErrorsProgramErrorValueMismatch: '',
    customErrorsProgramErrorValueMatch: '',
    customErrorsProgramErrorValueLess: '',
    customErrorsProgramErrorValueLessOrEqual: '',
};

/// Get the error message for a CustomErrorsProgram program error code.
String? getCustomErrorsProgramErrorMessage(int code) {
  return _customErrorsProgramErrorMessages[code];
}

/// Check if an error code belongs to the CustomErrorsProgram program.
bool isCustomErrorsProgramError(int code) {
  return _customErrorsProgramErrorMessages.containsKey(code);
}
