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
class CancelInstructionData {
  const CancelInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<CancelInstructionData> getCancelInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CancelInstructionData value) => <String, Object?>{'discriminator': 2},
  );
}

Decoder<CancelInstructionData> getCancelInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'cancel instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CancelInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (CancelInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CancelInstructionData>(
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
      VariableSizeDecoder<CancelInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CancelInstructionData, CancelInstructionData>
getCancelInstructionDataCodec() {
  return combineCodec(
    getCancelInstructionDataEncoder(),
    getCancelInstructionDataDecoder(),
  );
}

/// Creates a [Cancel] instruction.
Instruction getCancelInstruction({
  required Address programAddress,
  required Address admin,
  required Address mint,
  required Address vestingState,
  required Address vault,
  required Address tokenProgram,
}) {
  final instructionData = CancelInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.readonlySigner),
      AccountMeta(address: mint, role: AccountRole.readonly),
      AccountMeta(address: vestingState, role: AccountRole.writable),
      AccountMeta(address: vault, role: AccountRole.writable),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getCancelInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Cancel] instruction from raw instruction data.
CancelInstructionData parseCancelInstruction(Instruction instruction) {
  return getCancelInstructionDataDecoder().decode(instruction.data!);
}
