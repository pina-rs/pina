// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';


@immutable
class SpendingLimitUseInstructionData {
  const SpendingLimitUseInstructionData({
    required this.amount,
    required this.decimals,
  }) :
      discriminator = 13,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final BigInt amount;
  final int decimals;
}

Encoder<SpendingLimitUseInstructionData> getSpendingLimitUseInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('amount', getU64Encoder()),
    ('decimals', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (SpendingLimitUseInstructionData value) => <String, Object?>{
      'discriminator': 13,
      'migrationVersion': 0,
      'amount': value.amount,
      'decimals': value.decimals,
    },
  );
}

Decoder<SpendingLimitUseInstructionData> getSpendingLimitUseInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('amount', getU64Decoder()),
    ('decimals', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'spendingLimitUse instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (SpendingLimitUseInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(13),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      SpendingLimitUseInstructionData(
      amount: map['amount']! as BigInt,
      decimals: map['decimals']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SpendingLimitUseInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readTopLevel(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<SpendingLimitUseInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SpendingLimitUseInstructionData, SpendingLimitUseInstructionData> getSpendingLimitUseInstructionDataCodec() {
  return combineCodec(getSpendingLimitUseInstructionDataEncoder(), getSpendingLimitUseInstructionDataDecoder());
}

/// Creates a [SpendingLimitUse] instruction.
Instruction getSpendingLimitUseInstruction({
  required Address programAddress,
  required Address multisig,
  required Address spendingLimit,
  required Address member,
  required Address vault,
  required Address destination,
  required Address clock,
  Address? vaultTokenAccount,
  Address? mint,
  Address? tokenProgram,
  Address? systemProgram,
  required BigInt amount,
  required int decimals,
}) {
  final instructionData = SpendingLimitUseInstructionData(
      amount: amount,
      decimals: decimals,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: multisig, role: AccountRole.readonly),
    AccountMeta(address: spendingLimit, role: AccountRole.writable),
    AccountMeta(address: member, role: AccountRole.readonlySigner),
    AccountMeta(address: vault, role: AccountRole.writable),
    AccountMeta(address: destination, role: AccountRole.writable),
    AccountMeta(address: clock, role: AccountRole.readonly),
    if (vaultTokenAccount != null) AccountMeta(address: vaultTokenAccount, role: AccountRole.writable) else AccountMeta(address: programAddress, role: AccountRole.readonly),
    if (mint != null) AccountMeta(address: mint, role: AccountRole.readonly) else AccountMeta(address: programAddress, role: AccountRole.readonly),
    if (tokenProgram != null) AccountMeta(address: tokenProgram, role: AccountRole.readonly) else AccountMeta(address: programAddress, role: AccountRole.readonly),
    if (systemProgram != null) AccountMeta(address: systemProgram, role: AccountRole.readonly) else AccountMeta(address: programAddress, role: AccountRole.readonly),
    ],
    data: getSpendingLimitUseInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [SpendingLimitUse] instruction from raw instruction data.
SpendingLimitUseInstructionData parseSpendingLimitUseInstruction(Instruction instruction) {
  return getSpendingLimitUseInstructionDataDecoder().decode(instruction.data!);
}
