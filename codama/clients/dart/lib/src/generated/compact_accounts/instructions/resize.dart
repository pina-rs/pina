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
class ResizeInstructionData {
  const ResizeInstructionData({required this.entryCount}) : discriminator = 1;

  final int discriminator;
  final int entryCount;
}

Encoder<ResizeInstructionData> getResizeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('entryCount', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ResizeInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'entryCount': value.entryCount,
    },
  );
}

Decoder<ResizeInstructionData> getResizeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('entryCount', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'resize instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ResizeInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ResizeInstructionData(entryCount: map['entryCount']! as int),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ResizeInstructionData>(
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
      VariableSizeDecoder<ResizeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ResizeInstructionData, ResizeInstructionData>
getResizeInstructionDataCodec() {
  return combineCodec(
    getResizeInstructionDataEncoder(),
    getResizeInstructionDataDecoder(),
  );
}

/// Creates a [Resize] instruction.
Instruction getResizeInstruction({
  required Address programAddress,
  required Address authority,
  required Address journal,
  required Address systemProgram,
  required int entryCount,
}) {
  final instructionData = ResizeInstructionData(entryCount: entryCount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: journal, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getResizeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Resize] instruction from raw instruction data.
ResizeInstructionData parseResizeInstruction(Instruction instruction) {
  return getResizeInstructionDataDecoder().decode(instruction.data!);
}
