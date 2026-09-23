// Manual adjacent ABI migration for the SEC-02 regression.
// Source version: 0 (variable bytes)
// Destination version: 1 (variable bytes)
//
// The two versions share one physical shape; the transition is the identity
// on bytes. Only the envelope version byte differs, which the migration
// executor stamps itself.
pub(crate) fn target_size(data: &[u8]) -> Option<usize> {
	// Header: discriminator, version, label length, codes length.
	let label_len = usize::from(*data.get(2)?);
	let codes_len = usize::from(u16::from_le_bytes([*data.get(3)?, *data.get(4)?]));
	Some(5_usize.checked_add(label_len)?.checked_add(codes_len * 2)?)
}

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn working_size(data: &[u8], target_size: usize) -> Option<usize> {
	Some(data.len().max(target_size))
}

pub(crate) fn migrate(_data: &mut [u8]) {
	// Same-shape transition: no byte moves.
}
