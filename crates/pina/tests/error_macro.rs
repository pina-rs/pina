use pina::*;

#[error(crate = ::pina)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	Invalid = 0,
	Duplicate = 1,
}

#[test]
fn test_error_macro() {
	let error = MyError::Invalid;
	let program_error: ProgramError = error.into();
	match program_error {
		ProgramError::Custom(code) => assert_eq!(code, 0),
		_ => panic!("Wrong error type"),
	}

	let error = MyError::Duplicate;
	let program_error: ProgramError = error.into();
	match program_error {
		ProgramError::Custom(code) => assert_eq!(code, 1),
		_ => panic!("Wrong error type"),
	}
}
