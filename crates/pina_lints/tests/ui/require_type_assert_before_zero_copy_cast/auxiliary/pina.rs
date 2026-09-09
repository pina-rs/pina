pub trait ProcessAccountInfos {
	fn process(self, bytes: &[u8]) -> Result<(), ()>;
}
