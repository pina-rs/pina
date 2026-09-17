// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';

import 'event_log.dart';

/// Event record `PolicyChecked`.
class PolicyCheckedEvent {
	const PolicyCheckedEvent({
		required this.discriminator,
		required this.migrationVersion,
		required this.amount,
		required this.memo,
		required this.approvals,
		required this.requiredApprovals,
	});

	final int discriminator;
	final int migrationVersion;
	final BigInt amount;
	final String memo;
	final List<int> approvals;
	final int requiredApprovals;

	String get name => 'policyChecked';

	String toString() => 'PolicyCheckedEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, amount: ${amount}, memo: ${memo}, approvals: ${approvals}, requiredApprovals: ${requiredApprovals})';
}

/// The discriminator this event is emitted under.
const policyCheckedEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _policyCheckedEventDiscriminatorBytes = [1];

/// The version this client was generated from.
const policyCheckedEventMigrationVersion = 0;

/// Exact current byte length of a `PolicyChecked` record, envelope included.
const policyCheckedEventSize = 82;

/// Decode one current-version `PolicyChecked` record.
PolicyCheckedEvent decodePolicyCheckedEvent(Uint8List data) {
	if (data.length != policyCheckedEventSize) {
		throw RangeError('expected exactly ${policyCheckedEventSize} bytes, received ${data.length}');
	}
	var cursor = 0;
	final (v0, c0) = getU8Decoder().read(data, cursor);
	cursor = c0;
	if (v0 != 1) {
		throw RangeError('the provided bytes do not match the "PolicyChecked" event discriminator');
	}
	final (v1, c1) = getU8Decoder().read(data, cursor);
	cursor = c1;
	if (v1 != 0) {
		throw RangeError(
			v1 < 0
				? 'event migration version mismatch: expected 0, received $v1 (the log predates this client; project it through the checked-in event history or decode it with a client generated from the schema that wrote it)'
				: 'event migration version mismatch: expected 0, received $v1 (the log was written by a newer program; upgrade this client)',
		);
	}
	final (v2, c2) = getU64Decoder().read(data, cursor);
	cursor = c2;
	final (v3, c3) = fixDecoderSize(addDecoderSizePrefix(getUtf8Decoder(), getU8Decoder()), 65).read(data, cursor);
	cursor = c3;
	final (v4, c4) = fixDecoderSize(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(getU16Decoder())), 6).read(data, cursor);
	cursor = c4;
	final (v5, c5) = getU8Decoder().read(data, cursor);
	cursor = c5;

	return PolicyCheckedEvent(discriminator: v0, migrationVersion: v1, amount: v2, memo: v3, approvals: v4, requiredApprovals: v5);
}

/// Event bytes projected into the current shape.
class NormalizedPolicyCheckedEvent extends ValidationProgramEvent {
	const NormalizedPolicyCheckedEvent({
		required this.data,
		required this.sourceVersion,
		required this.wasMigrated,
	});

	final PolicyCheckedEvent data;

	/// The version carried by the immutable log record, matching the runtime's
	/// `CurrentEventData::source_version`.
	final int sourceVersion;

	/// Whether a historical projection ran.
	final bool wasMigrated;

	@override
	String get name => 'policyChecked';
}

/// Adjacent projections from the checked-in migration manifest: `(from, to,
/// automatic, source payload size, destination payload size, moves)`.
const List<(int, int, bool, int, int, List<(int, int, int)>)> _policyCheckedProjectionSteps = [

];

/// Project current or historical bytes into the current shape, mirroring the
/// runtime's `normalize_event_data`. Unknown, future, non-exact, and manual
/// transitions fail closed.
NormalizedPolicyCheckedEvent normalizePolicyCheckedEvent(Uint8List data) {
	if (data.length < 2) {
		throw RangeError('the provided data is too short for the "PolicyChecked" event envelope');
	}
	for (var index = 0; index < 1; index++) {
		if (data[index] != _policyCheckedEventDiscriminatorBytes[index]) {
			throw RangeError('the provided data does not match the "PolicyChecked" event discriminator');
		}
	}
	final sourceVersion = data[1];
	if (sourceVersion > 0) {
		throw RangeError(
			'event migration version mismatch: expected 0, received $sourceVersion (the log was written by a newer program; upgrade this client)',
		);
	}
	if (sourceVersion == 0) {
		return NormalizedPolicyCheckedEvent(
			data: decodePolicyCheckedEvent(data),
			sourceVersion: sourceVersion,
			wasMigrated: false,
		);
	}
	final projected = _projectPolicyCheckedEvent(data, sourceVersion);
	return NormalizedPolicyCheckedEvent(
		data: decodePolicyCheckedEvent(projected),
		sourceVersion: sourceVersion,
		wasMigrated: true,
	);
}

Uint8List _projectPolicyCheckedEvent(Uint8List data, int sourceVersion) {
	var version = sourceVersion;
	var payload = Uint8List.fromList(data.sublist(2));
	while (version != 0) {
		(int, int, bool, int, int, List<(int, int, int)>)? step;
		for (final candidate in _policyCheckedProjectionSteps) {
			if (candidate.$1 == version) {
				step = candidate;
				break;
			}
		}
		if (step == null) {
			throw RangeError(
				'event migration version mismatch: expected 0, received $version (this client has no checked-in projection for it)',
			);
		}
		if (!step.$3) {
			throw RangeError(
				'event migration version mismatch: expected 0, received ${step.$1} (the v${step.$1} to v${step.$2} transition is manual, so only an on-chain projection or a client generated from that schema can represent it)',
			);
		}
		if (payload.length != step.$4) {
			throw RangeError(
				'event migration version mismatch: expected 0, received $version (the log length does not match the v$version schema)',
			);
		}
		final destination = Uint8List(step.$5);
		for (final (sourceOffset, destinationOffset, size) in step.$6) {
			destination.setRange(destinationOffset, destinationOffset + size, payload, sourceOffset);
		}
		payload = destination;
		version = step.$2;
	}

	final projected = Uint8List(2 + payload.length);
	projected.setRange(0, 1, _policyCheckedEventDiscriminatorBytes);
	projected[1] = 0;
	projected.setRange(2, projected.length, payload);
	return projected;
}
/// A decoded `PolicyChecked` log record.
typedef DecodedPolicyCheckedEvent = NormalizedPolicyCheckedEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
NormalizedPolicyCheckedEvent? parsePolicyCheckedEventFromLog(String log) {
	final bytes = decodeProgramDataLog(log);
	if (bytes == null || bytes.length < 1) {
		return null;
	}
	for (var index = 0; index < 1; index++) {
		if (bytes[index] != _policyCheckedEventDiscriminatorBytes[index]) {
			return null;
		}
	}
	return normalizePolicyCheckedEvent(bytes);
}
