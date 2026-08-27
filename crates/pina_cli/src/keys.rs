//! Program ID inspection and explicit source synchronization.

use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use atomic_write_file::AtomicWriteFile;
use ed25519_dalek::SigningKey;
use proc_macro2::LineColumn;
use solana_address::Address;
use syn::LitStr;
use syn::visit::Visit;

use crate::project::Project;
use crate::project::ProjectError;

const MAX_KEYPAIR_FILE_SIZE: u64 = 4 * 1024;

/// A program ID declaration discovered in Rust source.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramIdDeclaration {
	/// Declared base58 program address.
	pub program_id: String,
	/// Source file containing the declaration.
	pub source: PathBuf,
}

/// Current source and keypair identity information.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInspection {
	/// Discovered program project.
	pub project: Project,
	/// Program ID currently declared in source.
	pub declared_program_id: String,
	/// Selected keypair path.
	pub keypair: PathBuf,
	/// Public key derived from the selected keypair, when it exists.
	pub keypair_program_id: Option<String>,
	/// Whether source and keypair identities match.
	pub matches: Option<bool>,
}

/// Result of synchronizing the source declaration.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySync {
	/// Source file that was inspected or changed.
	pub source: PathBuf,
	/// Program ID before synchronization.
	pub previous_program_id: String,
	/// Program ID derived from the keypair.
	pub program_id: String,
	/// Whether the source file changed.
	pub changed: bool,
}

/// Result of generating a local program identity.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyGeneration {
	/// Keypair file that was created or explicitly replaced.
	pub keypair: PathBuf,
	/// New program ID derived from the generated keypair.
	pub program_id: String,
	/// Source synchronization result.
	pub source: PathBuf,
}

/// Program-key inspection and synchronization failures.
#[derive(Debug, thiserror::Error)]
pub enum KeysError {
	/// Project discovery failed.
	#[error("project discovery failed: {0:?}")]
	Project(#[from] ProjectError),

	/// A source or keypair file could not be read.
	#[error("failed to read {path:?}: {source}")]
	Read {
		path: PathBuf,
		source: std::io::Error,
	},

	/// Rust source could not be parsed.
	#[error("failed to parse {path:?}: {source}")]
	ParseSource { path: PathBuf, source: syn::Error },

	/// Source must contain exactly one unambiguous declaration.
	#[error("expected exactly one declare_id! invocation in {path:?}, found {count}")]
	DeclarationCount { path: PathBuf, count: usize },

	/// The declared address is not a valid Solana address.
	#[error("invalid program ID {program_id:?} in {path:?}")]
	InvalidProgramId { path: PathBuf, program_id: String },

	/// Keypair JSON could not be decoded.
	#[error("invalid keypair JSON at {path:?}: {source}")]
	KeypairJson {
		path: PathBuf,
		source: serde_json::Error,
	},

	/// Keypair inputs are intentionally small, bounded regular files.
	#[error("keypair file exceeds the 4096-byte limit at {path:?}")]
	KeypairTooLarge { path: PathBuf },

	/// Keypair inputs must not be links, reparse points, or special files.
	#[error("refusing non-regular, symbolic-link, or reparse-point keypair input {path:?}")]
	UnsafeKeypairInput { path: PathBuf },

	/// Solana keypair files contain exactly 64 bytes.
	#[error("invalid keypair length at {path:?}: expected 64 bytes, found {length}")]
	KeypairLength { path: PathBuf, length: usize },

	/// The public half does not correspond to the secret signing key.
	#[error("keypair public key does not match its secret key at {path:?}")]
	KeypairMismatch { path: PathBuf },

	/// Existing keypair replacement requires explicit authorization.
	#[error("refusing to overwrite existing keypair {path:?}; pass --force to rotate identity")]
	DestinationExists { path: PathBuf },

	/// The generated file changed before it could be synchronized.
	#[error("generated keypair changed before synchronization at {path:?}")]
	GeneratedChanged { path: PathBuf },

	/// Keypair and source targets must be ordinary files.
	#[error("refusing non-regular, symbolic-link, or reparse-point destination {path:?}")]
	UnsafeDestination { path: PathBuf },

	/// Secure operating-system randomness was unavailable.
	#[error("failed to generate secure key material: {message:?}")]
	Random { message: String },

	/// The platform cannot guarantee private permissions for a new keypair.
	#[error("refusing to create a keypair because private file permissions are unsupported")]
	PrivatePermissionsUnsupported,

	/// A source update could not be written.
	#[error("failed to write {path:?}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},

	/// The program source changed after it was parsed.
	#[error("program source changed while synchronizing {path:?}; retry the command")]
	SourceChanged { path: PathBuf },

	/// A multi-file identity update failed and could not be fully rolled back.
	#[error("identity update failed: {operation}; rollback also failed: {rollback}")]
	Rollback { operation: String, rollback: String },
}

/// Inspect the program ID declared by a project and an optional local keypair.
pub fn inspect_keys(start: &Path, keypair: Option<&Path>) -> Result<KeyInspection, KeysError> {
	let project = Project::discover(start)?;
	let declaration = inspect_program_id(&project.library_source)?;
	let keypair = keypair.map_or_else(|| project.keypair(), Path::to_path_buf);
	let keypair_program_id = optional_metadata(&keypair, fs::symlink_metadata(&keypair))?
		.map(|_| read_keypair_program_id(&keypair))
		.transpose()?;
	let matches = keypair_program_id
		.as_ref()
		.map(|program_id| program_id == &declaration.program_id);

	Ok(KeyInspection {
		declared_program_id: declaration.program_id,
		project,
		keypair,
		keypair_program_id,
		matches,
	})
}

/// Synchronize the single source `declare_id!` with an explicit local keypair.
///
/// Only the string literal is replaced. The operation refuses ambiguous source,
/// malformed keypairs, and invalid addresses before writing any bytes.
pub fn sync_keys(start: &Path, keypair: Option<&Path>) -> Result<KeySync, KeysError> {
	let project = Project::discover(start)?;
	let default_keypair = project.keypair();
	let keypair = keypair.unwrap_or(&default_keypair);
	let program_id = read_keypair_program_id(keypair)?;
	let snapshot = read_project_source(&project)?;

	sync_source(&project, program_id, &snapshot, None)
}

fn sync_source(
	project: &Project,
	program_id: String,
	snapshot: &SourceSnapshot,
	generated: Option<(&Path, &same_file::Handle)>,
) -> Result<KeySync, KeysError> {
	let mut source = snapshot.contents.clone();
	let declaration = locate_declaration(&project.library_source, &source)?;
	let previous_program_id = declaration.value;

	if previous_program_id == program_id {
		if let Some((path, handle)) = generated {
			ensure_current_keypair_identity(path, handle)?;
		}

		return Ok(KeySync {
			source: project.library_source.clone(),
			previous_program_id,
			program_id,
			changed: false,
		});
	}

	// Safety: replacing only the parsed literal preserves every unrelated byte
	// and avoids reformatting the user's program source.
	source.replace_range(
		declaration.start..declaration.end,
		&format!("\"{program_id}\""),
	);
	write_source(
		&project.library_source,
		snapshot,
		source.as_bytes(),
		generated,
	)?;

	Ok(KeySync {
		source: project.library_source.clone(),
		previous_program_id,
		program_id,
		changed: true,
	})
}

/// Generate a fresh keypair and synchronize the source declaration.
///
/// Existing keypairs are never replaced unless `force` is true. The source is
/// parsed and validated before key material is generated or written.
pub fn generate_keys(
	start: &Path,
	keypair: Option<&Path>,
	force: bool,
) -> Result<KeyGeneration, KeysError> {
	#[cfg(not(windows))]
	ensure_private_keypair_support();

	#[cfg(windows)]
	ensure_private_keypair_support()?;

	let project = Project::discover(start)?;
	let keypair = keypair.map_or_else(|| project.keypair(), Path::to_path_buf);
	let source = read_project_source(&project)?;
	let _ = locate_declaration(&project.library_source, &source.contents)?;

	validate_destination(&keypair, force)?;
	let previous_keypair = optional_metadata(&keypair, fs::symlink_metadata(&keypair))?
		.map(|_| read_keypair_file(&keypair))
		.transpose()?;
	let mut secret = [0u8; 32];
	random_result(getrandom::fill(&mut secret))?;
	let signing_key = SigningKey::from_bytes(&secret);
	let public_key = signing_key.verifying_key().to_bytes();
	let program_id = Address::from(public_key).to_string();
	let mut bytes = Vec::with_capacity(64);
	bytes.extend(secret);
	bytes.extend(public_key);
	let json = encode_keypair(&bytes);

	let (generated_file, generated_handle) = write_keypair(&keypair, &json, force)?;
	finish_generated_keypair(
		project,
		&keypair,
		program_id,
		&source,
		&generated_handle,
		generated_file,
		previous_keypair.as_deref(),
	)
}

fn finish_generated_keypair(
	project: Project,
	keypair: &Path,
	program_id: String,
	source: &SourceSnapshot,
	generated_handle: &same_file::Handle,
	generated_file: fs::File,
	previous: Option<&[u8]>,
) -> Result<KeyGeneration, KeysError> {
	let sync = sync_source(
		&project,
		program_id.clone(),
		source,
		Some((keypair, generated_handle)),
	);

	if let Err(operation) = sync {
		let rollback = rollback_keypair(keypair, generated_handle, generated_file, previous);
		return finish_rollback(operation, rollback);
	}

	Ok(KeyGeneration {
		keypair: keypair.to_path_buf(),
		program_id,
		source: project.library_source,
	})
}

/// Inspect exactly one validated `declare_id!` in a Rust source file.
pub fn inspect_program_id(source: &Path) -> Result<ProgramIdDeclaration, KeysError> {
	let contents = read_result(source, fs::read_to_string(source))?;
	let declaration = locate_declaration(source, &contents)?;

	Ok(ProgramIdDeclaration {
		program_id: declaration.value,
		source: source.to_path_buf(),
	})
}

#[derive(Debug)]
struct SourceSnapshot {
	contents: String,
	handle: same_file::Handle,
}

fn read_project_source(project: &Project) -> Result<SourceSnapshot, KeysError> {
	let path = &project.library_source;
	let metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_source_metadata(path, &metadata)?;
	let mut file = read_result(path, fs::File::open(path))?;
	let opened_metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_source_metadata(path, &opened_metadata)?;
	let cloned = read_result(path, file.try_clone())?;
	let handle = read_result(path, same_file::Handle::from_file(cloned))?;
	let mut contents = String::new();
	read_result(path, file.read_to_string(&mut contents))?;
	let current = read_result(path, same_file::Handle::from_path(path))?;

	ensure_source_identity(path, &handle, &current)?;

	Ok(SourceSnapshot { contents, handle })
}

fn ensure_source_identity(
	path: &Path,
	expected: &same_file::Handle,
	current: &same_file::Handle,
) -> Result<(), KeysError> {
	if expected != current {
		return Err(KeysError::SourceChanged {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

pub(crate) fn read_keypair_program_id(path: &Path) -> Result<String, KeysError> {
	let contents = read_keypair_file(path)?;
	let bytes: Vec<u8> = serde_json::from_slice(&contents).map_err(|source| {
		KeysError::KeypairJson {
			path: path.to_path_buf(),
			source,
		}
	})?;

	if bytes.len() != 64 {
		return Err(KeysError::KeypairLength {
			path: path.to_path_buf(),
			length: bytes.len(),
		});
	}

	let mut secret = [0u8; 32];
	let mut public_key = [0u8; 32];
	secret.copy_from_slice(&bytes[..32]);
	public_key.copy_from_slice(&bytes[32..]);

	let derived = SigningKey::from_bytes(&secret).verifying_key().to_bytes();

	if derived != public_key {
		return Err(KeysError::KeypairMismatch {
			path: path.to_path_buf(),
		});
	}

	Ok(Address::from(derived).to_string())
}

fn read_keypair_file(path: &Path) -> Result<Vec<u8>, KeysError> {
	let metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_keypair_metadata(path, &metadata)?;
	let initial_handle = read_result(path, same_file::Handle::from_path(path))?;

	let file = read_result(path, fs::File::open(path))?;
	let opened_metadata = read_result(path, file.metadata())?;
	validate_keypair_metadata(path, &opened_metadata)?;

	ensure_file_identity(path, &initial_handle, &file)?;

	read_bounded_keypair(path, file, opened_metadata.len() as usize)
}

fn ensure_file_identity(
	path: &Path,
	initial: &same_file::Handle,
	opened: &fs::File,
) -> Result<(), KeysError> {
	// Recheck the path after both handles are open. This detects a link or file
	// replacement on every supported platform without unstable metadata APIs.
	let current_metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_keypair_metadata(path, &current_metadata)?;
	let current = read_result(path, same_file::Handle::from_path(path))?;
	let opened_file = read_result(path, opened.try_clone())?;
	let opened = read_result(path, same_file::Handle::from_file(opened_file))?;

	if initial != &opened || current != opened {
		return Err(KeysError::UnsafeKeypairInput {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn validate_keypair_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), KeysError> {
	if crate::path_security::is_link_like(metadata) || !metadata.is_file() {
		return Err(KeysError::UnsafeKeypairInput {
			path: path.to_path_buf(),
		});
	}

	if metadata.len() > MAX_KEYPAIR_FILE_SIZE {
		return Err(KeysError::KeypairTooLarge {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn read_bounded_keypair(
	path: &Path,
	reader: impl Read,
	capacity: usize,
) -> Result<Vec<u8>, KeysError> {
	let mut contents = Vec::with_capacity(capacity.min(MAX_KEYPAIR_FILE_SIZE as usize));
	let mut bounded = reader.take(MAX_KEYPAIR_FILE_SIZE + 1);
	read_result(path, bounded.read_to_end(&mut contents))?;

	if contents.len() as u64 > MAX_KEYPAIR_FILE_SIZE {
		return Err(KeysError::KeypairTooLarge {
			path: path.to_path_buf(),
		});
	}

	Ok(contents)
}

fn validate_destination(path: &Path, force: bool) -> Result<(), KeysError> {
	let has_link = read_result(path, crate::path_security::has_link_like_component(path))?;

	if has_link {
		return Err(KeysError::UnsafeDestination {
			path: path.to_path_buf(),
		});
	}

	let metadata = optional_metadata(path, fs::symlink_metadata(path))?;

	if metadata
		.as_ref()
		.is_some_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
	{
		return Err(KeysError::UnsafeDestination {
			path: path.to_path_buf(),
		});
	}

	if metadata.is_some() && !force {
		return Err(KeysError::DestinationExists {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

#[cfg(not(windows))]
fn ensure_private_keypair_support() {}

#[cfg(windows)]
fn ensure_private_keypair_support() -> Result<(), KeysError> {
	// Security: std does not expose Windows ACL construction. Creating the secret
	// and tightening an inherited ACL afterward would leave an exposure window.
	Err(KeysError::PrivatePermissionsUnsupported)
}

fn write_keypair(
	path: &Path,
	contents: &[u8],
	force: bool,
) -> Result<(fs::File, same_file::Handle), KeysError> {
	validate_destination(path, force)?;
	let parent = path.parent().unwrap_or_else(|| Path::new("."));
	write_result(parent, fs::create_dir_all(parent))?;
	let publication = prepare_atomic(path, contents, true)?;
	let published_file = read_result(path, publication.as_file().try_clone())?;
	let handle_file = read_result(path, published_file.try_clone())?;
	let published_handle = read_result(path, same_file::Handle::from_file(handle_file))?;
	write_result(path, publication.commit())?;
	ensure_current_keypair_identity(path, &published_handle)?;
	let mut verification_file = read_result(path, published_file.try_clone())?;
	read_result(path, verification_file.rewind())?;
	let verified = read_bounded_keypair(path, verification_file, contents.len())?;
	ensure_generated_contents(path, &verified, contents)?;

	Ok((published_file, published_handle))
}

fn ensure_generated_contents(
	path: &Path,
	verified: &[u8],
	expected: &[u8],
) -> Result<(), KeysError> {
	if verified != expected {
		return Err(KeysError::GeneratedChanged {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn validate_source_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), KeysError> {
	if crate::path_security::is_link_like(metadata) || !metadata.is_file() {
		return Err(KeysError::UnsafeDestination {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn write_source(
	path: &Path,
	snapshot: &SourceSnapshot,
	contents: &[u8],
	generated: Option<(&Path, &same_file::Handle)>,
) -> Result<(), KeysError> {
	let has_link = read_result(path, crate::path_security::has_link_like_component(path))?;

	if has_link {
		return Err(KeysError::UnsafeDestination {
			path: path.to_path_buf(),
		});
	}

	let metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_source_metadata(path, &metadata)?;
	let publication = prepare_atomic(path, contents, false)?;
	validate_source_snapshot(path, snapshot)?;

	if let Some((keypair, handle)) = generated {
		ensure_current_keypair_identity(keypair, handle)?;
	}

	write_result(path, publication.commit())
}

fn validate_source_snapshot(path: &Path, snapshot: &SourceSnapshot) -> Result<(), KeysError> {
	let metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_source_metadata(path, &metadata)?;
	let mut current_file = read_result(path, fs::File::open(path))?;
	let cloned = read_result(path, current_file.try_clone())?;
	let current_handle = read_result(path, same_file::Handle::from_file(cloned))?;
	let mut current_contents = Vec::new();
	read_result(path, current_file.read_to_end(&mut current_contents))?;
	let current_path_handle = read_result(path, same_file::Handle::from_path(path))?;

	if current_handle != snapshot.handle
		|| current_path_handle != snapshot.handle
		|| current_contents != snapshot.contents.as_bytes()
	{
		return Err(KeysError::SourceChanged {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn rollback_keypair(
	path: &Path,
	generated_handle: &same_file::Handle,
	_generated_file: fs::File,
	previous: Option<&[u8]>,
) -> Result<(), KeysError> {
	if let Some(previous) = previous {
		let publication = prepare_atomic(path, previous, true)?;
		ensure_current_keypair_identity(path, generated_handle)?;
		return write_result(path, publication.commit());
	}

	ensure_current_keypair_identity(path, generated_handle)?;
	write_result(path, fs::remove_file(path))
}

fn ensure_current_keypair_identity(
	path: &Path,
	generated_handle: &same_file::Handle,
) -> Result<(), KeysError> {
	let metadata = read_result(path, fs::symlink_metadata(path))?;
	validate_keypair_metadata(path, &metadata)?;
	let current = read_result(path, same_file::Handle::from_path(path))?;

	if &current != generated_handle {
		return Err(KeysError::UnsafeDestination {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

#[cfg(test)]
fn write_atomic(path: &Path, contents: &[u8], private: bool) -> Result<(), KeysError> {
	let file = prepare_atomic(path, contents, private)?;
	write_result(path, file.commit())
}

fn prepare_atomic(
	path: &Path,
	contents: &[u8],
	private: bool,
) -> Result<AtomicWriteFile, KeysError> {
	let mut options = AtomicWriteFile::options();
	options.read(true);
	let mut file = write_result(path, options.open(path))?;

	#[cfg(unix)]
	if private {
		use std::os::unix::fs::PermissionsExt;

		let permissions = fs::Permissions::from_mode(0o600);
		let result = file.as_file().set_permissions(permissions);
		write_result(path, result)?;
	}

	#[cfg(not(unix))]
	let _ = private;

	write_result(path, file.write_all(contents))?;
	Ok(file)
}

fn read_result<T>(path: &Path, result: std::io::Result<T>) -> Result<T, KeysError> {
	result.map_err(|source| {
		KeysError::Read {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn write_result<T>(path: &Path, result: std::io::Result<T>) -> Result<T, KeysError> {
	result.map_err(|source| {
		KeysError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn random_result(result: Result<(), getrandom::Error>) -> Result<(), KeysError> {
	result.map_err(|error| {
		KeysError::Random {
			message: error.to_string(),
		}
	})
}

fn optional_metadata(
	path: &Path,
	result: std::io::Result<fs::Metadata>,
) -> Result<Option<fs::Metadata>, KeysError> {
	match result {
		Ok(metadata) => Ok(Some(metadata)),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(source) => {
			Err(KeysError::Read {
				path: path.to_path_buf(),
				source,
			})
		}
	}
}

fn encode_keypair(bytes: &[u8]) -> Vec<u8> {
	// Solana keypair JSON is a fixed array of bytes. Direct encoding keeps this
	// step infallible, so a generated identity can fail only at a transactional
	// filesystem boundary.
	let values = bytes.iter().map(u8::to_string).collect::<Vec<_>>();
	format!("[{}]", values.join(",")).into_bytes()
}

fn finish_rollback<T>(
	operation: KeysError,
	rollback: Result<(), KeysError>,
) -> Result<T, KeysError> {
	match rollback {
		Ok(()) => Err(operation),
		Err(rollback) => {
			Err(KeysError::Rollback {
				operation: operation.to_string(),
				rollback: rollback.to_string(),
			})
		}
	}
}

struct LocatedDeclaration {
	value: String,
	start: usize,
	end: usize,
}

#[derive(Default)]
struct DeclarationVisitor {
	declarations: Vec<(LitStr, String)>,
}

impl<'ast> Visit<'ast> for DeclarationVisitor {
	fn visit_macro(&mut self, item: &'ast syn::Macro) {
		let is_declaration = item
			.path
			.segments
			.last()
			.is_some_and(|segment| segment.ident == "declare_id");

		if is_declaration && let Ok(literal) = syn::parse2::<LitStr>(item.tokens.clone()) {
			self.declarations.push((literal.clone(), literal.value()));
		}

		syn::visit::visit_macro(self, item);
	}
}

fn locate_declaration(path: &Path, source: &str) -> Result<LocatedDeclaration, KeysError> {
	let file = syn::parse_file(source).map_err(|source| {
		KeysError::ParseSource {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let mut visitor = DeclarationVisitor::default();
	visitor.visit_file(&file);

	if visitor.declarations.len() != 1 {
		return Err(KeysError::DeclarationCount {
			path: path.to_path_buf(),
			count: visitor.declarations.len(),
		});
	}

	let (literal, value) = visitor
		.declarations
		.pop()
		.expect("declaration count was checked");

	if Address::from_str(&value).is_err() {
		return Err(KeysError::InvalidProgramId {
			path: path.to_path_buf(),
			program_id: value,
		});
	}

	let span = literal.span();
	let start = byte_offset(source, span.start());
	let end = byte_offset(source, span.end());

	Ok(LocatedDeclaration { value, start, end })
}

fn byte_offset(source: &str, location: LineColumn) -> usize {
	let line_start = source
		.split_inclusive('\n')
		.take(location.line.saturating_sub(1))
		.map(str::len)
		.sum::<usize>();

	line_start + location.column
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	fn project(program_id: &str) -> TempDir {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::create_dir_all(temp.path().join("src"))
			.unwrap_or_else(|error| panic!("create failed: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"demo-program\"\n",
		)
		.unwrap_or_else(|error| panic!("write failed: {error}"));
		fs::write(
			temp.path().join("src/lib.rs"),
			format!("// keep me\nuse pina::*;\n\ndeclare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("write failed: {error}"));
		temp
	}

	fn keypair(path: &Path, secret: [u8; 32]) -> String {
		let public_key = SigningKey::from_bytes(&secret).verifying_key().to_bytes();
		let mut bytes = secret.to_vec();
		bytes.extend(public_key);
		fs::write(
			path,
			serde_json::to_vec(&bytes)
				.unwrap_or_else(|error| panic!("serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("write failed: {error}"));

		Address::from(public_key).to_string()
	}

	#[test]
	fn inspects_missing_keypair_without_failing() {
		let temp = project("11111111111111111111111111111111");

		let inspection = inspect_keys(temp.path(), None)
			.unwrap_or_else(|error| panic!("inspection failed: {error}"));

		assert_eq!(
			inspection.declared_program_id,
			"11111111111111111111111111111111"
		);
		assert_eq!(inspection.keypair_program_id, None);
		assert_eq!(inspection.matches, None);
	}

	#[test]
	fn inspection_uses_the_cargo_library_source() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"custom-source\"\n\n[lib]\npath = \"program.rs\"\n",
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(
			temp.path().join("program.rs"),
			"declare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

		let inspection = inspect_keys(temp.path(), None)
			.unwrap_or_else(|error| panic!("inspection failed: {error}"));

		assert!(
			same_file::is_same_file(
				&inspection.project.library_source,
				temp.path().join("program.rs"),
			)
			.unwrap_or_else(|error| panic!("source identity failed: {error}"))
		);
	}

	#[test]
	fn project_source_reads_preserve_the_metadata_path_on_failure() {
		let temp = project("11111111111111111111111111111111");
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		fs::remove_file(&project.library_source)
			.unwrap_or_else(|error| panic!("source remove failed: {error}"));

		let error = read_project_source(&project).expect_err("missing source must fail");

		assert!(matches!(error, KeysError::Read { path, .. } if path == project.library_source));
	}

	#[test]
	fn sync_replaces_only_the_program_id_literal() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = temp.path().join("program-keypair.json");
		let expected = keypair(&keypair_path, [9u8; 32]);

		let sync = sync_keys(temp.path(), Some(&keypair_path))
			.unwrap_or_else(|error| panic!("sync failed: {error}"));
		let source = fs::read_to_string(temp.path().join("src/lib.rs"))
			.unwrap_or_else(|error| panic!("read failed: {error}"));

		assert!(sync.changed);
		assert_eq!(sync.program_id, expected);
		assert!(source.starts_with("// keep me\nuse pina::*;\n\n"));
		assert!(source.contains(&format!("declare_id!(\"{}\")", sync.program_id)));
	}

	#[test]
	fn sync_reports_an_already_matching_identity() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = temp.path().join("program-keypair.json");
		let expected = keypair(&keypair_path, [8u8; 32]);
		fs::write(
			temp.path().join("src/lib.rs"),
			format!("declare_id!(\"{expected}\");\n"),
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

		let sync = sync_keys(temp.path(), Some(&keypair_path))
			.unwrap_or_else(|error| panic!("sync failed: {error}"));

		assert!(!sync.changed);
		assert_eq!(sync.previous_program_id, expected);
	}

	#[test]
	fn keypair_reader_rejects_missing_invalid_json_and_wrong_length() {
		let temp = project("11111111111111111111111111111111");
		let missing = temp.path().join("missing.json");
		let invalid = temp.path().join("invalid.json");
		let short = temp.path().join("short.json");
		let oversized = temp.path().join("oversized.json");
		let directory = temp.path().join("keypair-directory");
		fs::write(&invalid, "not-json")
			.unwrap_or_else(|error| panic!("invalid write failed: {error}"));
		fs::write(&short, "[1, 2, 3]")
			.unwrap_or_else(|error| panic!("short write failed: {error}"));
		fs::write(&oversized, vec![b'0'; MAX_KEYPAIR_FILE_SIZE as usize + 1])
			.unwrap_or_else(|error| panic!("oversized write failed: {error}"));
		fs::create_dir(&directory)
			.unwrap_or_else(|error| panic!("directory create failed: {error}"));

		assert!(matches!(
			read_keypair_program_id(&missing),
			Err(KeysError::Read { .. })
		));
		assert!(matches!(
			read_keypair_program_id(&invalid),
			Err(KeysError::KeypairJson { .. })
		));
		assert!(matches!(
			read_keypair_program_id(&short),
			Err(KeysError::KeypairLength { length: 3, .. })
		));
		assert!(matches!(
			read_keypair_program_id(&oversized),
			Err(KeysError::KeypairTooLarge { .. })
		));
		assert!(matches!(
			read_keypair_program_id(&directory),
			Err(KeysError::UnsafeKeypairInput { .. })
		));
		assert!(matches!(
			read_bounded_keypair(
				Path::new("growing-keypair.json"),
				&vec![0; MAX_KEYPAIR_FILE_SIZE as usize + 1][..],
				0,
			),
			Err(KeysError::KeypairTooLarge { .. })
		));
	}

	#[test]
	fn keypair_identity_comparison_detects_a_different_opened_file() {
		let temp = project("11111111111111111111111111111111");
		let first = temp.path().join("first.json");
		let second = temp.path().join("second.json");
		fs::write(&first, "first").unwrap_or_else(|error| panic!("first write failed: {error}"));
		fs::write(&second, "second").unwrap_or_else(|error| panic!("second write failed: {error}"));
		let first_handle = same_file::Handle::from_path(&first)
			.unwrap_or_else(|error| panic!("first handle failed: {error}"));
		let second_handle = same_file::Handle::from_path(&second)
			.unwrap_or_else(|error| panic!("second handle failed: {error}"));
		let first_file =
			fs::File::open(&first).unwrap_or_else(|error| panic!("first open failed: {error}"));
		let second_file =
			fs::File::open(&second).unwrap_or_else(|error| panic!("second open failed: {error}"));

		assert!(ensure_file_identity(&first, &first_handle, &first_file).is_ok());
		assert!(ensure_source_identity(&first, &first_handle, &first_handle).is_ok());
		assert!(matches!(
			ensure_source_identity(&first, &first_handle, &second_handle),
			Err(KeysError::SourceChanged { .. })
		));
		assert!(matches!(
			ensure_file_identity(&first, &first_handle, &second_file),
			Err(KeysError::UnsafeKeypairInput { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn keypair_reader_rejects_a_symbolic_link() {
		use std::os::unix::fs::symlink;

		let temp = project("11111111111111111111111111111111");
		let target = temp.path().join("real-keypair.json");
		let linked = temp.path().join("linked-keypair.json");
		keypair(&target, [4u8; 32]);
		symlink(&target, &linked).unwrap_or_else(|error| panic!("symlink failed: {error}"));

		assert!(matches!(
			read_keypair_program_id(&linked),
			Err(KeysError::UnsafeKeypairInput { .. })
		));
		assert!(matches!(
			inspect_keys(temp.path(), Some(&linked)),
			Err(KeysError::UnsafeKeypairInput { .. })
		));
	}

	#[test]
	fn filesystem_error_helpers_preserve_context_and_rollback_failures() {
		let path = Path::new("program-keypair.json");
		let denied = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let directory = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));

		assert!(matches!(
			write_result::<()>(path, Err(denied())),
			Err(KeysError::Write { .. })
		));
		assert!(matches!(
			optional_metadata(path, Err(denied())),
			Err(KeysError::Read { .. })
		));
		assert!(matches!(
			random_result(Err(getrandom::Error::UNSUPPORTED)),
			Err(KeysError::Random { .. })
		));
		assert!(matches!(
			ensure_generated_contents(path, b"changed", b"expected"),
			Err(KeysError::GeneratedChanged { .. })
		));
		ensure_generated_contents(path, b"expected", b"expected")
			.unwrap_or_else(|error| panic!("matching contents failed: {error}"));
		assert!(matches!(
			finish_rollback::<()>(
				KeysError::UnsafeDestination {
					path: path.to_path_buf(),
				},
				Err(KeysError::Write {
					path: path.to_path_buf(),
					source: denied(),
				}),
			),
			Err(KeysError::Rollback { .. })
		));
		assert!(matches!(
			finish_rollback::<()>(
				KeysError::UnsafeDestination {
					path: path.to_path_buf(),
				},
				Ok(()),
			),
			Err(KeysError::UnsafeDestination { .. })
		));
		assert!(matches!(
			write_source(
				&directory,
				&SourceSnapshot {
					contents: String::new(),
					handle: same_file::Handle::from_path(&directory)
						.unwrap_or_else(|error| panic!("directory handle failed: {error}")),
				},
				b"source",
				None,
			),
			Err(KeysError::UnsafeDestination { .. })
		));
	}

	#[test]
	fn source_sync_rejects_in_place_and_atomic_replacements() {
		let temp = project("11111111111111111111111111111111");
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let path = project.library_source.clone();
		let in_place = read_project_source(&project)
			.unwrap_or_else(|error| panic!("snapshot failed: {error}"));
		fs::write(
			&path,
			"// editor change\ndeclare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("in-place edit failed: {error}"));
		assert!(matches!(
			write_source(&path, &in_place, b"replacement", None),
			Err(KeysError::SourceChanged { .. })
		));
		assert!(
			fs::read_to_string(&path)
				.unwrap_or_else(|error| panic!("source read failed: {error}"))
				.contains("editor change")
		);

		let replaced = read_project_source(&project)
			.unwrap_or_else(|error| panic!("replacement snapshot failed: {error}"));
		write_atomic(&path, b"// atomic editor save\n", false)
			.unwrap_or_else(|error| panic!("atomic edit failed: {error}"));
		assert!(matches!(
			write_source(&path, &replaced, b"replacement", None),
			Err(KeysError::SourceChanged { .. })
		));
		assert_eq!(
			fs::read(&path).unwrap_or_else(|error| panic!("source read failed: {error}")),
			b"// atomic editor save\n"
		);

		#[cfg(unix)]
		{
			use std::os::unix::fs::symlink;

			let linked = temp.path().join("linked-source");
			symlink(temp.path().join("src"), &linked)
				.unwrap_or_else(|error| panic!("source link failed: {error}"));
			assert!(matches!(
				write_source(&linked.join("lib.rs"), &replaced, b"replacement", None),
				Err(KeysError::UnsafeDestination { .. })
			));
		}
	}

	#[test]
	fn rollback_refuses_to_mutate_a_replaced_generated_keypair() {
		let temp = project("11111111111111111111111111111111");
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let path = root.join("program-keypair.json");

		for previous in [None, Some(&b"previous identity"[..])] {
			let (generated_file, generated_handle) =
				write_keypair(&path, b"generated identity", true)
					.unwrap_or_else(|error| panic!("generated publication failed: {error}"));
			write_atomic(&path, b"concurrent replacement", true)
				.unwrap_or_else(|error| panic!("replacement failed: {error}"));

			assert!(matches!(
				rollback_keypair(&path, &generated_handle, generated_file, previous),
				Err(KeysError::UnsafeDestination { .. })
			));
			assert_eq!(
				fs::read(&path).unwrap_or_else(|error| panic!("replacement read failed: {error}")),
				b"concurrent replacement"
			);
		}
	}

	#[test]
	fn owned_generated_keypair_rollbacks_restore_or_remove_exactly_that_file() {
		let temp = project("11111111111111111111111111111111");
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let path = root.join("program-keypair.json");
		for previous in [None, Some(&b"previous identity"[..])] {
			let (file, handle) = write_keypair(&path, b"generated identity", true)
				.unwrap_or_else(|error| panic!("generated publication failed: {error}"));
			rollback_keypair(&path, &handle, file, previous)
				.unwrap_or_else(|error| panic!("rollback failed: {error}"));
			match previous {
				Some(contents) => {
					assert_eq!(
						fs::read(&path)
							.unwrap_or_else(|error| panic!("restored read failed: {error}")),
						contents
					);
				}
				None => assert!(!path.exists()),
			}
		}
	}

	#[test]
	fn generated_keypair_sync_failure_uses_identity_bound_rollback() {
		let temp = project("11111111111111111111111111111111");
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let path = root.join("generated-keypair.json");
		keypair(&path, [10u8; 32]);
		let bytes = fs::read(&path).unwrap_or_else(|error| panic!("keypair read failed: {error}"));
		let (file, handle) = write_keypair(&path, &bytes, true)
			.unwrap_or_else(|error| panic!("generated publication failed: {error}"));
		let source = read_project_source(&project)
			.unwrap_or_else(|error| panic!("source snapshot failed: {error}"));
		fs::write(&project.library_source, "not valid Rust (")
			.unwrap_or_else(|error| panic!("invalid source write failed: {error}"));

		let error = finish_generated_keypair(
			project,
			&path,
			"generated-program-id".to_owned(),
			&source,
			&handle,
			file,
			None,
		)
		.expect_err("source sync must fail");
		assert!(matches!(error, KeysError::SourceChanged { .. }));
		assert!(!path.exists());
	}

	#[test]
	fn generated_sync_uses_known_id_and_rejects_a_replaced_publication() {
		let temp = project("11111111111111111111111111111111");
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let source = read_project_source(&project)
			.unwrap_or_else(|error| panic!("source snapshot failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let keypair = root.join("generated-keypair.json");
		let (file, handle) = write_keypair(&keypair, b"generated identity", true)
			.unwrap_or_else(|error| panic!("generated publication failed: {error}"));
		let unchanged = sync_source(
			&project,
			"11111111111111111111111111111111".to_owned(),
			&source,
			Some((&keypair, &handle)),
		)
		.unwrap_or_else(|error| panic!("matching generated sync failed: {error}"));
		assert!(!unchanged.changed);
		let expected = Address::from([7u8; 32]).to_string();
		let source = read_project_source(&project)
			.unwrap_or_else(|error| panic!("second source snapshot failed: {error}"));
		write_atomic(&keypair, b"replacement identity", true)
			.unwrap_or_else(|error| panic!("replacement failed: {error}"));

		let error = sync_source(
			&project,
			expected.clone(),
			&source,
			Some((&keypair, &handle)),
		)
		.expect_err("replacement must invalidate the generated publication");
		assert!(matches!(error, KeysError::UnsafeDestination { .. }));
		assert_eq!(
			inspect_program_id(&project.library_source)
				.unwrap_or_else(|error| panic!("source inspection failed: {error}"))
				.program_id,
			"11111111111111111111111111111111"
		);
		assert_ne!(
			expected,
			read_keypair_program_id(&keypair).unwrap_or_default()
		);
		drop(file);
	}

	#[test]
	fn source_reader_rejects_missing_malformed_and_invalid_declarations() {
		let temp = project("11111111111111111111111111111111");
		let source = temp.path().join("src/lib.rs");
		let missing = temp.path().join("missing.rs");

		assert!(matches!(
			inspect_program_id(&missing),
			Err(KeysError::Read { .. })
		));

		fs::write(&source, "pub fn malformed(\n")
			.unwrap_or_else(|error| panic!("malformed write failed: {error}"));
		assert!(matches!(
			inspect_program_id(&source),
			Err(KeysError::ParseSource { .. })
		));

		fs::write(&source, "pub fn no_declaration() {}\n")
			.unwrap_or_else(|error| panic!("missing declaration write failed: {error}"));
		assert!(matches!(
			inspect_program_id(&source),
			Err(KeysError::DeclarationCount { count: 0, .. })
		));

		fs::write(&source, "declare_id!(\"invalid\\n\\u{1b}\");\n")
			.unwrap_or_else(|error| panic!("invalid ID write failed: {error}"));
		let error = inspect_program_id(&source).expect_err("invalid ID must fail");
		assert!(matches!(&error, KeysError::InvalidProgramId { .. }));
		let diagnostic = error.to_string();
		assert!(!diagnostic.contains('\n'));
		assert!(!diagnostic.contains('\u{1b}'));
		assert!(diagnostic.contains("\\n"));
		assert!(diagnostic.contains("\\u{1b}"));
	}

	#[test]
	fn inspection_reports_matching_and_mismatching_keypairs() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = temp.path().join("program-keypair.json");
		let program_id = keypair(&keypair_path, [6u8; 32]);

		let mismatch = inspect_keys(temp.path(), Some(&keypair_path))
			.unwrap_or_else(|error| panic!("mismatch inspection failed: {error}"));
		assert_eq!(mismatch.matches, Some(false));

		fs::write(
			temp.path().join("src/lib.rs"),
			format!("declare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));
		let matching = inspect_keys(temp.path(), Some(&keypair_path))
			.unwrap_or_else(|error| panic!("matching inspection failed: {error}"));
		assert_eq!(matching.matches, Some(true));
	}

	#[test]
	fn sync_refuses_ambiguous_declarations_without_writing() {
		let temp = project("11111111111111111111111111111111");
		let source_path = temp.path().join("src/lib.rs");
		let original =
			fs::read_to_string(&source_path).unwrap_or_else(|error| panic!("read failed: {error}"));
		fs::write(
			&source_path,
			format!("{original}\ndeclare_id!(\"11111111111111111111111111111111\");\n"),
		)
		.unwrap_or_else(|error| panic!("write failed: {error}"));
		let before =
			fs::read_to_string(&source_path).unwrap_or_else(|error| panic!("read failed: {error}"));
		let keypair_path = temp.path().join("program-keypair.json");
		keypair(&keypair_path, [3u8; 32]);

		let error = sync_keys(temp.path(), Some(&keypair_path))
			.expect_err("ambiguous declarations must fail closed");

		assert!(matches!(
			error,
			KeysError::DeclarationCount { count: 2, .. }
		));
		assert_eq!(
			fs::read_to_string(&source_path)
				.unwrap_or_else(|read_error| panic!("read failed: {read_error}")),
			before
		);
	}

	#[test]
	fn sync_rejects_inconsistent_keypair_before_writing() {
		let temp = project("11111111111111111111111111111111");
		let source_path = temp.path().join("src/lib.rs");
		let before =
			fs::read_to_string(&source_path).unwrap_or_else(|error| panic!("read failed: {error}"));
		let keypair_path = temp.path().join("corrupt-keypair.json");
		let mut corrupt = vec![7u8; 32];
		corrupt.extend([9u8; 32]);
		fs::write(
			&keypair_path,
			serde_json::to_vec(&corrupt)
				.unwrap_or_else(|error| panic!("serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("write failed: {error}"));

		let error = sync_keys(temp.path(), Some(&keypair_path))
			.expect_err("inconsistent keypair must fail closed");

		assert!(matches!(error, KeysError::KeypairMismatch { .. }));
		assert_eq!(
			fs::read_to_string(&source_path)
				.unwrap_or_else(|read_error| panic!("read failed: {read_error}")),
			before
		);
	}

	#[cfg(not(windows))]
	#[test]
	fn generation_refuses_existing_keypair_without_force() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"))
			.join("existing-keypair.json");
		let before_id = keypair(&keypair_path, [2u8; 32]);

		let error = generate_keys(temp.path(), Some(&keypair_path), false)
			.expect_err("existing identity must require force");

		assert!(matches!(error, KeysError::DestinationExists { .. }));
		assert_eq!(
			read_keypair_program_id(&keypair_path)
				.unwrap_or_else(|read_error| panic!("read failed: {read_error}")),
			before_id
		);
	}

	#[cfg(not(windows))]
	#[test]
	fn forced_generation_refuses_an_oversized_rollback_input() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"))
			.join("oversized-keypair.json");
		let bytes = vec![b'0'; MAX_KEYPAIR_FILE_SIZE as usize + 1];
		fs::write(&keypair_path, &bytes)
			.unwrap_or_else(|error| panic!("keypair write failed: {error}"));

		let error = generate_keys(temp.path(), Some(&keypair_path), true)
			.expect_err("oversized rollback input must fail before rotation");

		assert!(matches!(error, KeysError::KeypairTooLarge { .. }));
		assert_eq!(
			fs::read(&keypair_path)
				.unwrap_or_else(|read_error| panic!("keypair read failed: {read_error}")),
			bytes
		);
	}

	#[cfg(unix)]
	#[test]
	fn generation_refuses_a_symlinked_keypair_ancestor() {
		use std::os::unix::fs::symlink;

		let temp = project("11111111111111111111111111111111");
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let real = root.join("real-keys");
		let linked = root.join("linked-keys");
		fs::create_dir(&real).unwrap_or_else(|error| panic!("directory create failed: {error}"));
		symlink(&real, &linked).unwrap_or_else(|error| panic!("symlink failed: {error}"));
		let keypair_path = linked.join("program-keypair.json");

		let error = generate_keys(temp.path(), Some(&keypair_path), false)
			.expect_err("symlinked keypair ancestor must fail closed");

		assert!(matches!(error, KeysError::UnsafeDestination { .. }));
		assert!(!real.join("program-keypair.json").exists());
	}

	#[cfg(not(windows))]
	#[test]
	fn generation_validates_source_and_destination_before_writing() {
		let temp = project("11111111111111111111111111111111");
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let source = root.join("src/lib.rs");
		let keypair_path = root.join("program-keypair.json");
		fs::remove_file(&source).unwrap_or_else(|error| panic!("source remove failed: {error}"));

		assert!(matches!(
			generate_keys(&root, Some(&keypair_path), false),
			Err(KeysError::Project { .. })
		));
		assert!(!keypair_path.exists());

		fs::write(
			&source,
			"declare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("source restore failed: {error}"));
		fs::create_dir(&keypair_path)
			.unwrap_or_else(|error| panic!("destination directory failed: {error}"));
		assert!(matches!(
			generate_keys(&root, Some(&keypair_path), true),
			Err(KeysError::UnsafeDestination { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn generation_removes_a_new_keypair_when_source_commit_fails() {
		use std::os::unix::fs::symlink;

		let temp = project("11111111111111111111111111111111");
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let source = root.join("src/lib.rs");
		let target = root.join("real-lib.rs");
		fs::rename(&source, &target).unwrap_or_else(|error| panic!("source move failed: {error}"));
		symlink(&target, &source).unwrap_or_else(|error| panic!("source symlink failed: {error}"));
		let keypair_path = root.join("new-keypair.json");

		let error = generate_keys(&root, Some(&keypair_path), false)
			.expect_err("source publication must fail");

		assert!(matches!(error, KeysError::UnsafeDestination { .. }));
		assert!(!keypair_path.exists());
	}

	#[cfg(windows)]
	#[test]
	fn generation_fails_closed_without_private_acl_support() {
		let temp = project("11111111111111111111111111111111");
		let keypair_path = temp.path().join("program-keypair.json");

		let error = generate_keys(temp.path(), Some(&keypair_path), false)
			.expect_err("Windows key generation must fail closed");

		assert!(matches!(error, KeysError::PrivatePermissionsUnsupported));
		assert!(!keypair_path.exists());
	}

	#[cfg(unix)]
	#[test]
	fn forced_generation_rolls_back_keypair_when_source_commit_fails() {
		use std::os::unix::fs::symlink;

		let temp = project("11111111111111111111111111111111");
		let source_path = temp.path().join("src/lib.rs");
		let source_target = temp.path().join("real-lib.rs");
		fs::rename(&source_path, &source_target)
			.unwrap_or_else(|error| panic!("source move failed: {error}"));
		symlink(&source_target, &source_path)
			.unwrap_or_else(|error| panic!("source symlink failed: {error}"));
		let keypair_path = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"))
			.join("existing-keypair.json");
		keypair(&keypair_path, [5u8; 32]);
		let before =
			fs::read(&keypair_path).unwrap_or_else(|error| panic!("keypair read failed: {error}"));

		let error = generate_keys(temp.path(), Some(&keypair_path), true)
			.expect_err("unsafe source destination must fail");

		assert!(matches!(error, KeysError::UnsafeDestination { .. }));
		assert_eq!(
			fs::read(&keypair_path)
				.unwrap_or_else(|read_error| panic!("keypair read failed: {read_error}")),
			before
		);
	}
}
