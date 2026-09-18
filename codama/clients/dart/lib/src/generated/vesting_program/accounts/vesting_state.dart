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
class VestingState {
  const VestingState({
    required this.admin,
    required this.beneficiary,
    required this.mint,
    required this.totalAmount,
    required this.claimedAmount,
    required this.startTs,
    required this.cliffTs,
    required this.endTs,
    required this.cancelled,
    required this.bump,
  }) : discriminator = 1,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Address admin;
  final Address beneficiary;
  final Address mint;
  final BigInt totalAmount;
  final BigInt claimedAmount;
  final BigInt startTs;
  final BigInt cliffTs;
  final BigInt endTs;
  final bool cancelled;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VestingState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          admin == other.admin &&
          beneficiary == other.beneficiary &&
          mint == other.mint &&
          totalAmount == other.totalAmount &&
          claimedAmount == other.claimedAmount &&
          startTs == other.startTs &&
          cliffTs == other.cliffTs &&
          endTs == other.endTs &&
          cancelled == other.cancelled &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(
    discriminator,
    migrationVersion,
    admin,
    beneficiary,
    mint,
    totalAmount,
    claimedAmount,
    startTs,
    cliffTs,
    endTs,
    cancelled,
    bump,
  );

  @override
  String toString() =>
      'VestingState(discriminator: $discriminator, migrationVersion: $migrationVersion, admin: $admin, beneficiary: $beneficiary, mint: $mint, totalAmount: $totalAmount, claimedAmount: $claimedAmount, startTs: $startTs, cliffTs: $cliffTs, endTs: $endTs, cancelled: $cancelled, bump: $bump)';
}

Encoder<VestingState> getVestingStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('admin', getAddressEncoder()),
    ('beneficiary', getAddressEncoder()),
    ('mint', getAddressEncoder()),
    ('totalAmount', getU64Encoder()),
    ('claimedAmount', getU64Encoder()),
    ('startTs', getU64Encoder()),
    ('cliffTs', getU64Encoder()),
    ('endTs', getU64Encoder()),
    ('cancelled', getBooleanEncoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (VestingState value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'admin': value.admin,
      'beneficiary': value.beneficiary,
      'mint': value.mint,
      'totalAmount': value.totalAmount,
      'claimedAmount': value.claimedAmount,
      'startTs': value.startTs,
      'cliffTs': value.cliffTs,
      'endTs': value.endTs,
      'cancelled': value.cancelled,
      'bump': value.bump,
    },
  );
}

Decoder<VestingState> getVestingStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('admin', getAddressDecoder()),
    ('beneficiary', getAddressDecoder()),
    ('mint', getAddressDecoder()),
    ('totalAmount', getU64Decoder()),
    ('claimedAmount', getU64Decoder()),
    ('startTs', getU64Decoder()),
    ('cliffTs', getU64Decoder()),
    ('endTs', getU64Decoder()),
    ('cancelled', getBooleanDecoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'vestingState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (VestingState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
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
      VestingState(
        admin: map['admin']! as Address,
        beneficiary: map['beneficiary']! as Address,
        mint: map['mint']! as Address,
        totalAmount: map['totalAmount']! as BigInt,
        claimedAmount: map['claimedAmount']! as BigInt,
        startTs: map['startTs']! as BigInt,
        cliffTs: map['cliffTs']! as BigInt,
        endTs: map['endTs']! as BigInt,
        cancelled: map['cancelled']! as bool,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<VestingState>(
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
      VariableSizeDecoder<VestingState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<VestingState, VestingState> getVestingStateCodec() {
  return combineCodec(getVestingStateEncoder(), getVestingStateDecoder());
}

Account<VestingState> decodeVestingState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getVestingStateDecoder());
}

/// The account schema version this client was generated from.
const int vestingStateMigrationVersion = 0;

/// Cheap envelope check for fetched `VestingState` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool vestingStateNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 1) {
    return false;
  }
  return data[1] < 0;
}
