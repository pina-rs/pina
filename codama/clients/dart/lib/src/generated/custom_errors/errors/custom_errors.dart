// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the CustomErrors program.

const int customErrorsErrorHello = 0x1770; // 6000

const int customErrorsErrorHelloNoMsg = 0x17eb; // 6123

const int customErrorsErrorHelloNext = 0x17ec; // 6124

const int customErrorsErrorHelloCustom = 0x17ed; // 6125

const int customErrorsErrorValueMismatch = 0x17ee; // 6126

const int customErrorsErrorValueMatch = 0x17ef; // 6127

const int customErrorsErrorValueLess = 0x17f0; // 6128

const int customErrorsErrorValueLessOrEqual = 0x17f1; // 6129

/// Map of error codes to human-readable messages.
const Map<int, String> _customErrorsErrorMessages = {
  customErrorsErrorHello: '',
  customErrorsErrorHelloNoMsg: '',
  customErrorsErrorHelloNext: '',
  customErrorsErrorHelloCustom: '',
  customErrorsErrorValueMismatch: '',
  customErrorsErrorValueMatch: '',
  customErrorsErrorValueLess: '',
  customErrorsErrorValueLessOrEqual: '',
};

/// Get the error message for a CustomErrors program error code.
String? getCustomErrorsErrorMessage(int code) {
  return _customErrorsErrorMessages[code];
}

/// Check if an error code belongs to the CustomErrors program.
bool isCustomErrorsError(int code) {
  return _customErrorsErrorMessages.containsKey(code);
}
