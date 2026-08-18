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
class DepositInstructionData {
  const DepositInstructionData({required this.amount}) : discriminator = 2;

  final int discriminator;
  final BigInt amount;
}

Encoder<DepositInstructionData> getDepositInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (DepositInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'amount': value.amount,
    },
  );
}

Decoder<DepositInstructionData> getDepositInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'deposit instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (DepositInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      DepositInstructionData(amount: map['amount']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<DepositInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readExact(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<DepositInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DepositInstructionData, DepositInstructionData>
getDepositInstructionDataCodec() {
  return combineCodec(
    getDepositInstructionDataEncoder(),
    getDepositInstructionDataDecoder(),
  );
}

/// Creates a [Deposit] instruction.
Instruction getDepositInstruction({
  required Address programAddress,
  required Address user,
  required Address stakeMint,
  required Address poolState,
  required Address positionState,
  required Address userStakeAta,
  required Address associatedTokenProgram,
  required Address tokenProgram,
  required Address systemProgram,
  required BigInt amount,
}) {
  final instructionData = DepositInstructionData(amount: amount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: user, role: AccountRole.writableSigner),
      AccountMeta(address: stakeMint, role: AccountRole.readonly),
      AccountMeta(address: poolState, role: AccountRole.writable),
      AccountMeta(address: positionState, role: AccountRole.writable),
      AccountMeta(address: userStakeAta, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getDepositInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Deposit] instruction from raw instruction data.
DepositInstructionData parseDepositInstruction(Instruction instruction) {
  return getDepositInstructionDataDecoder().decode(instruction.data!);
}
