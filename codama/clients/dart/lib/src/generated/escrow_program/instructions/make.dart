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
class MakeInstructionData {
  const MakeInstructionData({
    required this.seed,
    required this.amountA,
    required this.amountB,
    required this.bump,
  }) : discriminator = 1;

  final int discriminator;
  final BigInt seed;
  final BigInt amountA;
  final BigInt amountB;
  final int bump;
}

Encoder<MakeInstructionData> getMakeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('seed', getU64Encoder()),
    ('amountA', getU64Encoder()),
    ('amountB', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (MakeInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'seed': value.seed,
      'amountA': value.amountA,
      'amountB': value.amountB,
      'bump': value.bump,
    },
  );
}

Decoder<MakeInstructionData> getMakeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('seed', getU64Decoder()),
    ('amountA', getU64Decoder()),
    ('amountB', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'make instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (MakeInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      MakeInstructionData(
        seed: map['seed']! as BigInt,
        amountA: map['amountA']! as BigInt,
        amountB: map['amountB']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<MakeInstructionData>(
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
      VariableSizeDecoder<MakeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<MakeInstructionData, MakeInstructionData> getMakeInstructionDataCodec() {
  return combineCodec(
    getMakeInstructionDataEncoder(),
    getMakeInstructionDataDecoder(),
  );
}

/// Creates a [Make] instruction.
Instruction getMakeInstruction({
  required Address programAddress,
  required Address maker,
  required Address mintA,
  required Address mintB,
  required Address makerAtaA,
  required Address escrow,
  required Address vault,
  required Address associatedTokenProgram,
  required Address systemProgram,
  required Address tokenProgram,
  required BigInt seed,
  required BigInt amountA,
  required BigInt amountB,
  required int bump,
}) {
  final instructionData = MakeInstructionData(
    seed: seed,
    amountA: amountA,
    amountB: amountB,
    bump: bump,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: maker, role: AccountRole.writableSigner),
      AccountMeta(address: mintA, role: AccountRole.readonly),
      AccountMeta(address: mintB, role: AccountRole.readonly),
      AccountMeta(address: makerAtaA, role: AccountRole.writable),
      AccountMeta(address: escrow, role: AccountRole.writable),
      AccountMeta(address: vault, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getMakeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Make] instruction from raw instruction data.
MakeInstructionData parseMakeInstruction(Instruction instruction) {
  return getMakeInstructionDataDecoder().decode(instruction.data!);
}
