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
class AllowsDuplicateMutableInstructionData {
  const AllowsDuplicateMutableInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<AllowsDuplicateMutableInstructionData>
getAllowsDuplicateMutableInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AllowsDuplicateMutableInstructionData value) => <String, Object?>{
      'discriminator': 1,
    },
  );
}

Decoder<AllowsDuplicateMutableInstructionData>
getAllowsDuplicateMutableInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'allowsDuplicateMutable instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (AllowsDuplicateMutableInstructionData, int) readExact(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (AllowsDuplicateMutableInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AllowsDuplicateMutableInstructionData>(
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
      VariableSizeDecoder<AllowsDuplicateMutableInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<
  AllowsDuplicateMutableInstructionData,
  AllowsDuplicateMutableInstructionData
>
getAllowsDuplicateMutableInstructionDataCodec() {
  return combineCodec(
    getAllowsDuplicateMutableInstructionDataEncoder(),
    getAllowsDuplicateMutableInstructionDataDecoder(),
  );
}

/// Creates a [AllowsDuplicateMutable] instruction.
Instruction getAllowsDuplicateMutableInstruction({
  required Address programAddress,
}) {
  final instructionData = AllowsDuplicateMutableInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getAllowsDuplicateMutableInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [AllowsDuplicateMutable] instruction from raw instruction data.
AllowsDuplicateMutableInstructionData parseAllowsDuplicateMutableInstruction(
  Instruction instruction,
) {
  return getAllowsDuplicateMutableInstructionDataDecoder().decode(
    instruction.data!,
  );
}
