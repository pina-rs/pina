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
class OpenPositionInstructionData {
  const OpenPositionInstructionData({required this.bump}) : discriminator = 1;

  final int discriminator;
  final int bump;
}

Encoder<OpenPositionInstructionData> getOpenPositionInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (OpenPositionInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
    },
  );
}

Decoder<OpenPositionInstructionData> getOpenPositionInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'openPosition instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (OpenPositionInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (OpenPositionInstructionData(bump: map['bump']! as int), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<OpenPositionInstructionData>(
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
      VariableSizeDecoder<OpenPositionInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<OpenPositionInstructionData, OpenPositionInstructionData>
getOpenPositionInstructionDataCodec() {
  return combineCodec(
    getOpenPositionInstructionDataEncoder(),
    getOpenPositionInstructionDataDecoder(),
  );
}

/// Creates a [OpenPosition] instruction.
Instruction getOpenPositionInstruction({
  required Address programAddress,
  required Address user,
  required Address poolState,
  required Address positionState,
  required Address systemProgram,
  required int bump,
}) {
  final instructionData = OpenPositionInstructionData(bump: bump);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: user, role: AccountRole.writableSigner),
      AccountMeta(address: poolState, role: AccountRole.readonly),
      AccountMeta(address: positionState, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getOpenPositionInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [OpenPosition] instruction from raw instruction data.
OpenPositionInstructionData parseOpenPositionInstruction(
  Instruction instruction,
) {
  return getOpenPositionInstructionDataDecoder().decode(instruction.data!);
}
