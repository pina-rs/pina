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
class AllowsDuplicateReadonlyInstructionData {
  const AllowsDuplicateReadonlyInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<AllowsDuplicateReadonlyInstructionData>
getAllowsDuplicateReadonlyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AllowsDuplicateReadonlyInstructionData value) => <String, Object?>{
      'discriminator': 2,
    },
  );
}

Decoder<AllowsDuplicateReadonlyInstructionData>
getAllowsDuplicateReadonlyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'allowsDuplicateReadonly instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (AllowsDuplicateReadonlyInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (AllowsDuplicateReadonlyInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AllowsDuplicateReadonlyInstructionData>(
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
      VariableSizeDecoder<AllowsDuplicateReadonlyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<
  AllowsDuplicateReadonlyInstructionData,
  AllowsDuplicateReadonlyInstructionData
>
getAllowsDuplicateReadonlyInstructionDataCodec() {
  return combineCodec(
    getAllowsDuplicateReadonlyInstructionDataEncoder(),
    getAllowsDuplicateReadonlyInstructionDataDecoder(),
  );
}

/// Creates a [AllowsDuplicateReadonly] instruction.
Instruction getAllowsDuplicateReadonlyInstruction({
  required Address programAddress,
  required Address account1,
  required Address account2,
}) {
  final instructionData = AllowsDuplicateReadonlyInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: account1, role: AccountRole.readonly),
      AccountMeta(address: account2, role: AccountRole.readonly),
    ],
    data: getAllowsDuplicateReadonlyInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [AllowsDuplicateReadonly] instruction from raw instruction data.
AllowsDuplicateReadonlyInstructionData parseAllowsDuplicateReadonlyInstruction(
  Instruction instruction,
) {
  return getAllowsDuplicateReadonlyInstructionDataDecoder().decode(
    instruction.data!,
  );
}
