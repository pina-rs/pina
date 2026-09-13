// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';

import 'event_log.dart';

/// Event record `PolicyChecked`.
class PolicyCheckedEvent extends ValidationProgramEvent {
  const PolicyCheckedEvent({
    required this.discriminator,
    required this.amount,
    required this.memo,
    required this.approvals,
    required this.requiredApprovals,
  });

  final int discriminator;
  final BigInt amount;
  final String memo;
  final List<int> approvals;
  final int requiredApprovals;

  @override
  String get name => 'policyChecked';

  String toString() =>
      'PolicyCheckedEvent(discriminator: ${discriminator}, amount: ${amount}, memo: ${memo}, approvals: ${approvals}, requiredApprovals: ${requiredApprovals})';
}

/// The discriminator this event is emitted under.
const policyCheckedEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _policyCheckedEventDiscriminatorBytes = [1];

/// Exact current byte length of a `PolicyChecked` record, envelope included.
const policyCheckedEventSize = 81;

/// Decode one current-version `PolicyChecked` record.
PolicyCheckedEvent decodePolicyCheckedEvent(Uint8List data) {
  if (data.length != policyCheckedEventSize) {
    throw RangeError(
      'expected exactly ${policyCheckedEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 1) {
    throw RangeError(
      'the provided bytes do not match the "PolicyChecked" event discriminator',
    );
  }
  final (v1, c1) = getU64Decoder().read(data, cursor);
  cursor = c1;
  final (v2, c2) = fixDecoderSize(
    addDecoderSizePrefix(getUtf8Decoder(), getU8Decoder()),
    65,
  ).read(data, cursor);
  cursor = c2;
  final (v3, c3) = fixDecoderSize(
    getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(getU16Decoder())),
    6,
  ).read(data, cursor);
  cursor = c3;
  final (v4, c4) = getU8Decoder().read(data, cursor);
  cursor = c4;

  return PolicyCheckedEvent(
    discriminator: v0,
    amount: v1,
    memo: v2,
    approvals: v3,
    requiredApprovals: v4,
  );
}

/// A decoded `PolicyChecked` log record.
typedef DecodedPolicyCheckedEvent = PolicyCheckedEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
PolicyCheckedEvent? parsePolicyCheckedEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 1) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _policyCheckedEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  return decodePolicyCheckedEvent(bytes);
}
