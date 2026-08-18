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
class TakeInstructionData {
  const TakeInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<TakeInstructionData> getTakeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (TakeInstructionData value) => <String, Object?>{'discriminator': 2},
  );
}

Decoder<TakeInstructionData> getTakeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'take instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TakeInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (TakeInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<TakeInstructionData>(
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
      VariableSizeDecoder<TakeInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TakeInstructionData, TakeInstructionData> getTakeInstructionDataCodec() {
  return combineCodec(
    getTakeInstructionDataEncoder(),
    getTakeInstructionDataDecoder(),
  );
}

/// Creates a [Take] instruction.
Instruction getTakeInstruction({
  required Address programAddress,
  required Address taker,
  required Address mintA,
  required Address mintB,
  required Address takerAtaA,
  required Address takerAtaB,
  required Address maker,
  required Address makerAtaB,
  required Address escrow,
  required Address vault,
  required Address tokenProgram,
  required Address associatedTokenProgram,
  required Address systemProgram,
}) {
  final instructionData = TakeInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: taker, role: AccountRole.writableSigner),
      AccountMeta(address: mintA, role: AccountRole.readonly),
      AccountMeta(address: mintB, role: AccountRole.readonly),
      AccountMeta(address: takerAtaA, role: AccountRole.writable),
      AccountMeta(address: takerAtaB, role: AccountRole.writable),
      AccountMeta(address: maker, role: AccountRole.writable),
      AccountMeta(address: makerAtaB, role: AccountRole.writable),
      AccountMeta(address: escrow, role: AccountRole.writable),
      AccountMeta(address: vault, role: AccountRole.writable),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getTakeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Take] instruction from raw instruction data.
TakeInstructionData parseTakeInstruction(Instruction instruction) {
  return getTakeInstructionDataDecoder().decode(instruction.data!);
}
