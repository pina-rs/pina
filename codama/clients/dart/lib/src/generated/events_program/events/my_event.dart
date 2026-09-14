// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `MyEvent`.
class MyEventEvent extends EventsProgramEvent {
	const MyEventEvent({
		required this.discriminator,
		required this.data,
		required this.label,
	});

	final int discriminator;
	final BigInt data;
	final Uint8List label;

	@override
	String get name => 'myEvent';

	String toString() => 'MyEventEvent(discriminator: ${discriminator}, data: ${data}, label: ${label})';
}

/// The discriminator this event is emitted under.
const myEventEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _myEventEventDiscriminatorBytes = [1];

/// Exact current byte length of a `MyEvent` record, envelope included.
const myEventEventSize = 17;

/// Decode one current-version `MyEvent` record.
MyEventEvent decodeMyEventEvent(Uint8List data) {
	if (data.length != myEventEventSize) {
		throw RangeError('expected exactly ${myEventEventSize} bytes, received ${data.length}');
	}
	var cursor = 0;
	final (v0, c0) = getU8Decoder().read(data, cursor);
	cursor = c0;
	if (v0 != 1) {
		throw RangeError('the provided bytes do not match the "MyEvent" event discriminator');
	}
	final (v1, c1) = getU64Decoder().read(data, cursor);
	cursor = c1;
	final (v2, c2) = fixDecoderSize(getBytesDecoder(), 8).read(data, cursor);
	cursor = c2;

	return MyEventEvent(discriminator: v0, data: v1, label: v2);
}

/// A decoded `MyEvent` log record.
typedef DecodedMyEventEvent = MyEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
MyEventEvent? parseMyEventEventFromLog(String log) {
	final bytes = decodeProgramDataLog(log);
	if (bytes == null || bytes.length < 1) {
		return null;
	}
	for (var index = 0; index < 1; index++) {
		if (bytes[index] != _myEventEventDiscriminatorBytes[index]) {
			return null;
		}
	}
	return decodeMyEventEvent(bytes);
}
