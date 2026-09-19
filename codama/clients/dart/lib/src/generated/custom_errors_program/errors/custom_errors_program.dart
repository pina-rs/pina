// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CustomErrorsProgram program.

/// A plain custom error.
/// Message: "A plain custom error."
const int customErrorsProgramErrorHello = 0x1770; // 6000

const int customErrorsProgramErrorHelloNoMsg = 0x17eb; // 6123

/// A distinct code with its own message.
/// Message: "A distinct code with its own message."
const int customErrorsProgramErrorHelloNext = 0x17ec; // 6124

/// A custom error carrying caller-supplied context.
/// Message: "A custom error carrying caller-supplied context."
const int customErrorsProgramErrorHelloCustom = 0x17ed; // 6125

/// Two values were expected to differ and did not.
/// Message: "Two values were expected to differ and did not."
const int customErrorsProgramErrorValueMismatch = 0x17ee; // 6126

/// Two values were expected to be equal and were not.
/// Message: "Two values were expected to be equal and were not."
const int customErrorsProgramErrorValueMatch = 0x17ef; // 6127

/// The compared value was not less than the bound.
/// Message: "The compared value was not less than the bound."
const int customErrorsProgramErrorValueLess = 0x17f0; // 6128

/// The compared value was not less than or equal to the bound.
/// Message: "The compared value was not less than or equal to the bound."
const int customErrorsProgramErrorValueLessOrEqual = 0x17f1; // 6129

/// Map of error codes to human-readable messages.
const Map<int, String> _customErrorsProgramErrorMessages = {
  customErrorsProgramErrorHello: 'A plain custom error.',
  customErrorsProgramErrorHelloNoMsg: '',
  customErrorsProgramErrorHelloNext: 'A distinct code with its own message.',
  customErrorsProgramErrorHelloCustom:
      'A custom error carrying caller-supplied context.',
  customErrorsProgramErrorValueMismatch:
      'Two values were expected to differ and did not.',
  customErrorsProgramErrorValueMatch:
      'Two values were expected to be equal and were not.',
  customErrorsProgramErrorValueLess:
      'The compared value was not less than the bound.',
  customErrorsProgramErrorValueLessOrEqual:
      'The compared value was not less than or equal to the bound.',
};

/// Get the error message for a CustomErrorsProgram program error code.
String? getCustomErrorsProgramErrorMessage(int code) {
  return _customErrorsProgramErrorMessages[code];
}

/// Check if an error code belongs to the CustomErrorsProgram program.
bool isCustomErrorsProgramError(int code) {
  return _customErrorsProgramErrorMessages.containsKey(code);
}
