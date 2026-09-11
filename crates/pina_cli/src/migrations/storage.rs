//! Atomic file writes, migration locking, and hashing primitives.

use std::fmt::Write as _;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;

use atomic_write_file::AtomicWriteFile;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha256;

use super::MigrationError;

pub(super) fn read_bytes(path: &Path) -> Result<Vec<u8>, MigrationError> {
	ensure_safe_path(path)?;
	std::fs::read(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})
}

pub(super) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), MigrationError> {
	ensure_safe_path(path)?;
	let parent = path.parent().ok_or_else(|| {
		MigrationError::InvalidHistory(format!("{} has no parent directory", path.display()))
	})?;
	std::fs::create_dir_all(parent).map_err(|source| {
		MigrationError::CreateDirectory {
			path: parent.to_path_buf(),
			source,
		}
	})?;
	ensure_safe_path(path)?;
	let mut bytes = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
	value.serialize(&mut serializer).map_err(|source| {
		MigrationError::SerializeJson {
			path: path.to_path_buf(),
			source,
		}
	})?;
	bytes.push(b'\n');
	write_atomic(path, &bytes)
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), MigrationError> {
	ensure_safe_path(path)?;
	let mut file = AtomicWriteFile::open(path).map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})?;
	write_all(&mut file, bytes, path)?;
	file.commit().map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

pub(super) fn write_all(
	mut writer: impl std::io::Write,
	bytes: &[u8],
	path: &Path,
) -> Result<(), MigrationError> {
	writer.write_all(bytes).map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

#[derive(Debug)]
pub(super) struct MigrationLock(File);

impl Drop for MigrationLock {
	fn drop(&mut self) {
		let _ = fs2::FileExt::unlock(&self.0);
	}
}

pub(super) fn acquire_migration_lock(program_dir: &Path) -> Result<MigrationLock, MigrationError> {
	let directory = program_dir.join("migrations");
	ensure_safe_path(&directory)?;
	std::fs::create_dir_all(&directory).map_err(|source| {
		MigrationError::CreateDirectory {
			path: directory.clone(),
			source,
		}
	})?;
	let path = directory.join(".lock");
	ensure_safe_path(&path)?;
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(&path)
		.map_err(|source| {
			MigrationError::Lock {
				path: path.clone(),
				source,
			}
		})?;
	fs2::FileExt::lock_exclusive(&file).map_err(|source| MigrationError::Lock { path, source })?;
	Ok(MigrationLock(file))
}

pub(super) fn ensure_safe_path(path: &Path) -> Result<(), MigrationError> {
	if crate::path_security::has_link_like_component(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})? {
		return Err(MigrationError::UnsafePath {
			path: path.to_path_buf(),
		});
	}
	Ok(())
}

pub(super) fn hash_regular_file(path: &Path) -> Result<[u8; 32], MigrationError> {
	ensure_safe_path(path)?;
	let metadata = std::fs::symlink_metadata(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})?;
	if !metadata.is_file() {
		return Err(MigrationError::InvalidHistory(format!(
			"publication artifact {} is not a regular file",
			path.display()
		)));
	}
	let mut file = File::open(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})?;
	hash_reader(path, &mut file)
}

pub(super) fn hash_reader(
	path: &Path,
	mut reader: impl std::io::Read,
) -> Result<[u8; 32], MigrationError> {
	let mut digest = Sha256::new();
	let mut buffer = vec![0_u8; 64 * 1024];
	loop {
		let read = reader.read(&mut buffer).map_err(|source| {
			MigrationError::Read {
				path: path.to_path_buf(),
				source,
			}
		})?;
		if read == 0 {
			break;
		}
		digest.update(&buffer[..read]);
	}
	Ok(digest.finalize().into())
}

pub(super) fn hex_digest(digest: [u8; 32]) -> String {
	let mut output = String::with_capacity(64);
	for byte in digest {
		let _ = write!(output, "{byte:02x}");
	}
	output
}
