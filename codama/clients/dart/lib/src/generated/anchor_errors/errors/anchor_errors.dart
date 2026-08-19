// Auto-generated. Do not edit.
// ignore_for_file: type=lint, constant_identifier_names

/// Error codes for the AnchorErrors program.

const int anchorErrorsErrorHello = 0x1770; // 6000

const int anchorErrorsErrorHelloNoMsg = 0x17eb; // 6123

const int anchorErrorsErrorHelloNext = 0x17ec; // 6124

const int anchorErrorsErrorHelloCustom = 0x17ed; // 6125

const int anchorErrorsErrorValueMismatch = 0x17ee; // 6126

const int anchorErrorsErrorValueMatch = 0x17ef; // 6127

const int anchorErrorsErrorValueLess = 0x17f0; // 6128

const int anchorErrorsErrorValueLessOrEqual = 0x17f1; // 6129

/// Map of error codes to human-readable messages.
const Map<int, String> _anchorErrorsErrorMessages = {
  anchorErrorsErrorHello: '',
  anchorErrorsErrorHelloNoMsg: '',
  anchorErrorsErrorHelloNext: '',
  anchorErrorsErrorHelloCustom: '',
  anchorErrorsErrorValueMismatch: '',
  anchorErrorsErrorValueMatch: '',
  anchorErrorsErrorValueLess: '',
  anchorErrorsErrorValueLessOrEqual: '',
};

/// Get the error message for a AnchorErrors program error code.
String? getAnchorErrorsErrorMessage(int code) {
  return _anchorErrorsErrorMessages[code];
}

/// Check if an error code belongs to the AnchorErrors program.
bool isAnchorErrorsError(int code) {
  return _anchorErrorsErrorMessages.containsKey(code);
}
