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
class WithdrawInstructionData {
  const WithdrawInstructionData({required this.amount}) : discriminator = 3;

  final int discriminator;
  final BigInt amount;
}

Encoder<WithdrawInstructionData> getWithdrawInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (WithdrawInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'amount': value.amount,
    },
  );
}

Decoder<WithdrawInstructionData> getWithdrawInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'withdraw instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (WithdrawInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      WithdrawInstructionData(amount: map['amount']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<WithdrawInstructionData>(
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
      VariableSizeDecoder<WithdrawInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<WithdrawInstructionData, WithdrawInstructionData>
getWithdrawInstructionDataCodec() {
  return combineCodec(
    getWithdrawInstructionDataEncoder(),
    getWithdrawInstructionDataDecoder(),
  );
}

/// Creates a [Withdraw] instruction.
Instruction getWithdrawInstruction({
  required Address programAddress,
  required Address user,
  required Address stakeMint,
  required Address poolState,
  required Address positionState,
  required Address userStakeAta,
  required Address tokenProgram,
  required Address systemProgram,
  required BigInt amount,
}) {
  final instructionData = WithdrawInstructionData(amount: amount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: user, role: AccountRole.readonlySigner),
      AccountMeta(address: stakeMint, role: AccountRole.readonly),
      AccountMeta(address: poolState, role: AccountRole.writable),
      AccountMeta(address: positionState, role: AccountRole.writable),
      AccountMeta(address: userStakeAta, role: AccountRole.writable),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getWithdrawInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Withdraw] instruction from raw instruction data.
WithdrawInstructionData parseWithdrawInstruction(Instruction instruction) {
  return getWithdrawInstructionDataDecoder().decode(instruction.data!);
}
