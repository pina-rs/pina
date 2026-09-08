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
class Realloc2InstructionData {
  const Realloc2InstructionData({required this.len}) : discriminator = 1;

  final int discriminator;
  final int len;
}

Encoder<Realloc2InstructionData> getRealloc2InstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('len', getU16Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (Realloc2InstructionData value) => <String, Object?>{
      'discriminator': 1,
      'len': value.len,
    },
  );
}

Decoder<Realloc2InstructionData> getRealloc2InstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('len', getU16Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'realloc2 instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (Realloc2InstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (Realloc2InstructionData(len: map['len']! as int), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<Realloc2InstructionData>(
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
      VariableSizeDecoder<Realloc2InstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<Realloc2InstructionData, Realloc2InstructionData>
getRealloc2InstructionDataCodec() {
  return combineCodec(
    getRealloc2InstructionDataEncoder(),
    getRealloc2InstructionDataDecoder(),
  );
}

/// Creates a [Realloc2] instruction.
Instruction getRealloc2Instruction({
  required Address programAddress,
  required Address authority,
  required Address sample1,
  required Address sample2,
  required Address systemProgram,
  required int len,
}) {
  final instructionData = Realloc2InstructionData(len: len);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: sample1, role: AccountRole.writable),
      AccountMeta(address: sample2, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getRealloc2InstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Realloc2] instruction from raw instruction data.
Realloc2InstructionData parseRealloc2Instruction(Instruction instruction) {
  return getRealloc2InstructionDataDecoder().decode(instruction.data!);
}
