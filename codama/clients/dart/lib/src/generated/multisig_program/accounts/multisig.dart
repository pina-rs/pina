// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import '../pina_pod_codecs.dart';
import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';


@immutable
class Multisig {
  const Multisig({
    required this.bump,
    required this.createKey,
    required this.configAuthority,
    required this.rentCollector,
    required this.threshold,
    required this.timelock,
    required this.ttl,
    required this.transactionIndex,
    required this.staleTransactionIndex,
    required this.memberRoster,
    required this.memberPermissions,
  }) :
      discriminator = 2,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address createKey;
  final Address configAuthority;
  final Address rentCollector;
  final int threshold;
  final int timelock;
  final int ttl;
  final BigInt transactionIndex;
  final BigInt staleTransactionIndex;
  final List<int> memberRoster;
  final List<int> memberPermissions;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Multisig &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          createKey == other.createKey &&
          configAuthority == other.configAuthority &&
          rentCollector == other.rentCollector &&
          threshold == other.threshold &&
          timelock == other.timelock &&
          ttl == other.ttl &&
          transactionIndex == other.transactionIndex &&
          staleTransactionIndex == other.staleTransactionIndex &&
          memberRoster == other.memberRoster &&
          memberPermissions == other.memberPermissions;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, createKey, configAuthority, rentCollector, threshold, timelock, ttl, transactionIndex, staleTransactionIndex, memberRoster, memberPermissions);

  @override
  String toString() => 'Multisig(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, createKey: $createKey, configAuthority: $configAuthority, rentCollector: $rentCollector, threshold: $threshold, timelock: $timelock, ttl: $ttl, transactionIndex: $transactionIndex, staleTransactionIndex: $staleTransactionIndex, memberRoster: $memberRoster, memberPermissions: $memberPermissions)';
}


Encoder<Multisig> getMultisigEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('createKey', getAddressEncoder()),
    ('configAuthority', getAddressEncoder()),
    ('rentCollector', getAddressEncoder()),
    ('threshold', getU16Encoder()),
    ('timelock', getU32Encoder()),
    ('ttl', getU32Encoder()),
    ('transactionIndex', getU64Encoder()),
    ('staleTransactionIndex', getU64Encoder()),
    ('memberRoster', offsetEncoder(getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU16Encoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 512), OffsetConfig(preOffset: (scope) => scope.preOffset + 3))),
    ('memberPermissions', getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU8Encoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 16)),
  ]);

  return transformEncoder(
    structEncoder,
    (Multisig value) => <String, Object?>{
      'discriminator': 2,
      'migrationVersion': 0,
      'bump': value.bump,
      'createKey': value.createKey,
      'configAuthority': value.configAuthority,
      'rentCollector': value.rentCollector,
      'threshold': value.threshold,
      'timelock': value.timelock,
      'ttl': value.ttl,
      'transactionIndex': value.transactionIndex,
      'staleTransactionIndex': value.staleTransactionIndex,
      'memberRoster': value.memberRoster,
      'memberPermissions': value.memberPermissions,
    },
  );
}

Decoder<Multisig> getMultisigDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('createKey', getAddressDecoder()),
    ('configAuthority', getAddressDecoder()),
    ('rentCollector', getAddressDecoder()),
    ('threshold', getU16Decoder()),
    ('timelock', getU32Decoder()),
    ('ttl', getU32Decoder()),
    ('transactionIndex', getU64Decoder()),
    ('staleTransactionIndex', getU64Decoder()),
    ('memberRoster', offsetDecoder(getPinaPodBoundedArrayDecoder(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 512), 512), OffsetConfig(preOffset: (scope) => scope.preOffset + 3))),
    ('memberPermissions', getPinaPodBoundedArrayDecoder(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU8Decoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU8Decoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 16), 16)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'multisig account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (Multisig, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(2),
    ).read(bytes, offset + 0);
    final (storedMigrationVersion, _) = getU8Decoder().read(bytes, offset + 1);
    if (storedMigrationVersion != 0) {
      throw StateError(
        storedMigrationVersion < 0
            ? 'migration version mismatch: expected 0, received $storedMigrationVersion (the data predates this client; migrate it by sending a transaction to the program, or decode it with a client generated from an older IDL)'
            : 'migration version mismatch: expected 0, received $storedMigrationVersion (the data was written by a newer program; upgrade this client)',
      );
    }
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      Multisig(
      bump: map['bump']! as int,
      createKey: map['createKey']! as Address,
      configAuthority: map['configAuthority']! as Address,
      rentCollector: map['rentCollector']! as Address,
      threshold: map['threshold']! as int,
      timelock: map['timelock']! as int,
      ttl: map['ttl']! as int,
      transactionIndex: map['transactionIndex']! as BigInt,
      staleTransactionIndex: map['staleTransactionIndex']! as BigInt,
      memberRoster: map['memberRoster']! as List<int>,
      memberPermissions: map['memberPermissions']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<Multisig>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength < structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readTopLevel(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<Multisig>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<Multisig, Multisig> getMultisigCodec() {
  return combineCodec(getMultisigEncoder(), getMultisigDecoder());
}

Account<Multisig> decodeMultisig(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getMultisigDecoder());
}

/// The account schema version this client was generated from.
const int multisigMigrationVersion = 0;

/// Cheap envelope check for fetched `Multisig` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool multisigNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 2) {
		return false;
	}
	return data[1] < 0;
}
