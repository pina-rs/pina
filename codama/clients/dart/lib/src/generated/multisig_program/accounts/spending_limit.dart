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
class SpendingLimit {
  const SpendingLimit({
    required this.bump,
    required this.multisig,
    required this.createKey,
    required this.vaultIndex,
    required this.mint,
    required this.amount,
    required this.remainingAmount,
    required this.lastReset,
    required this.period,
    required this.members,
    required this.destinations,
  }) :
      discriminator = 4,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address multisig;
  final Address createKey;
  final int vaultIndex;
  final Address mint;
  final BigInt amount;
  final BigInt remainingAmount;
  final BigInt lastReset;
  final int period;
  final List<int> members;
  final List<int> destinations;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SpendingLimit &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          multisig == other.multisig &&
          createKey == other.createKey &&
          vaultIndex == other.vaultIndex &&
          mint == other.mint &&
          amount == other.amount &&
          remainingAmount == other.remainingAmount &&
          lastReset == other.lastReset &&
          period == other.period &&
          members == other.members &&
          destinations == other.destinations;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, multisig, createKey, vaultIndex, mint, amount, remainingAmount, lastReset, period, members, destinations);

  @override
  String toString() => 'SpendingLimit(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, multisig: $multisig, createKey: $createKey, vaultIndex: $vaultIndex, mint: $mint, amount: $amount, remainingAmount: $remainingAmount, lastReset: $lastReset, period: $period, members: $members, destinations: $destinations)';
}


Encoder<SpendingLimit> getSpendingLimitEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('multisig', getAddressEncoder()),
    ('createKey', getAddressEncoder()),
    ('vaultIndex', getU8Encoder()),
    ('mint', getAddressEncoder()),
    ('amount', getU64Encoder()),
    ('remainingAmount', getU64Encoder()),
    ('lastReset', getI64Encoder()),
    ('period', getU8Encoder()),
    ('members', offsetEncoder(getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU16Encoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 512), OffsetConfig(preOffset: (scope) => scope.preOffset + 4))),
    ('destinations', getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU16Encoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 256)),
  ]);

  return transformEncoder(
    structEncoder,
    (SpendingLimit value) => <String, Object?>{
      'discriminator': 4,
      'migrationVersion': 0,
      'bump': value.bump,
      'multisig': value.multisig,
      'createKey': value.createKey,
      'vaultIndex': value.vaultIndex,
      'mint': value.mint,
      'amount': value.amount,
      'remainingAmount': value.remainingAmount,
      'lastReset': value.lastReset,
      'period': value.period,
      'members': value.members,
      'destinations': value.destinations,
    },
  );
}

Decoder<SpendingLimit> getSpendingLimitDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('multisig', getAddressDecoder()),
    ('createKey', getAddressDecoder()),
    ('vaultIndex', getU8Decoder()),
    ('mint', getAddressDecoder()),
    ('amount', getU64Decoder()),
    ('remainingAmount', getU64Decoder()),
    ('lastReset', getI64Decoder()),
    ('period', getU8Decoder()),
    ('members', offsetDecoder(getPinaPodBoundedArrayDecoder(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 125)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 512), 512), OffsetConfig(preOffset: (scope) => scope.preOffset + 4))),
    ('destinations', getPinaPodBoundedArrayDecoder(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 127)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 256), 256)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'spendingLimit account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (SpendingLimit, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(4),
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
      SpendingLimit(
      bump: map['bump']! as int,
      multisig: map['multisig']! as Address,
      createKey: map['createKey']! as Address,
      vaultIndex: map['vaultIndex']! as int,
      mint: map['mint']! as Address,
      amount: map['amount']! as BigInt,
      remainingAmount: map['remainingAmount']! as BigInt,
      lastReset: map['lastReset']! as BigInt,
      period: map['period']! as int,
      members: map['members']! as List<int>,
      destinations: map['destinations']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SpendingLimit>(
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
      VariableSizeDecoder<SpendingLimit>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SpendingLimit, SpendingLimit> getSpendingLimitCodec() {
  return combineCodec(getSpendingLimitEncoder(), getSpendingLimitDecoder());
}

Account<SpendingLimit> decodeSpendingLimit(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getSpendingLimitDecoder());
}

/// The account schema version this client was generated from.
const int spendingLimitMigrationVersion = 0;

/// Cheap envelope check for fetched `SpendingLimit` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool spendingLimitNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 4) {
		return false;
	}
	return data[1] < 0;
}
