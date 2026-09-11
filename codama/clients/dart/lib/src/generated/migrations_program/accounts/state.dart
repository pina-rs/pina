// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';

@immutable
class State {
  const State({
    required this.authority,
    required this.value,
    required this.enabled,
    required this.revision,
  }) : discriminator = 1,
       migrationVersion = 2;

  final int discriminator;
  final int migrationVersion;
  final Address authority;
  final BigInt value;
  final bool enabled;
  final int revision;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is State &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          authority == other.authority &&
          value == other.value &&
          enabled == other.enabled &&
          revision == other.revision;

  @override
  int get hashCode => Object.hash(
    discriminator,
    migrationVersion,
    authority,
    value,
    enabled,
    revision,
  );

  @override
  String toString() =>
      'State(discriminator: $discriminator, migrationVersion: $migrationVersion, authority: $authority, value: $value, enabled: $enabled, revision: $revision)';
}

Encoder<State> getStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('authority', getAddressEncoder()),
    ('value', getU64Encoder()),
    ('enabled', getBooleanEncoder()),
    ('revision', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (State value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 2,
      'authority': value.authority,
      'value': value.value,
      'enabled': value.enabled,
      'revision': value.revision,
    },
  );
}

Decoder<State> getStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('authority', getAddressDecoder()),
    ('value', getU64Decoder()),
    ('enabled', getBooleanDecoder()),
    ('revision', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'state account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (State, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      State(
        authority: map['authority']! as Address,
        value: map['value']! as BigInt,
        enabled: map['enabled']! as bool,
        revision: map['revision']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<State>(
      fixedSize: structDecoder.fixedSize,
      read: (bytes, offset) {
        final bytesLength = bytes.length - offset;
        if (bytesLength < structDecoder.fixedSize) {
          throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
        }
        return readTopLevel(bytes, offset);
      },
    ),
    VariableSizeDecoder<Map<String, Object?>>() => VariableSizeDecoder<State>(
      read: readTopLevel,
      maxSize: structDecoder.maxSize,
    ),
  };
}

Codec<State, State> getStateCodec() {
  return combineCodec(getStateEncoder(), getStateDecoder());
}

Account<State> decodeState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getStateDecoder());
}

/// The account schema version this client was generated from.
const int stateMigrationVersion = 2;

/// Cheap envelope check for fetched `State` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool stateNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 1) {
    return false;
  }
  return data[1] < 2;
}
