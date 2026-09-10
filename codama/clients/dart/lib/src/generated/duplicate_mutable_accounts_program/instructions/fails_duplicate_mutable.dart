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
class FailsDuplicateMutableInstructionData {
  const FailsDuplicateMutableInstructionData() : discriminator = 0;

  final int discriminator;
}

Encoder<FailsDuplicateMutableInstructionData>
getFailsDuplicateMutableInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (FailsDuplicateMutableInstructionData value) => <String, Object?>{
      'discriminator': 0,
    },
  );
}

Decoder<FailsDuplicateMutableInstructionData>
getFailsDuplicateMutableInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'failsDuplicateMutable instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (FailsDuplicateMutableInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (FailsDuplicateMutableInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<FailsDuplicateMutableInstructionData>(
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
      VariableSizeDecoder<FailsDuplicateMutableInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<
  FailsDuplicateMutableInstructionData,
  FailsDuplicateMutableInstructionData
>
getFailsDuplicateMutableInstructionDataCodec() {
  return combineCodec(
    getFailsDuplicateMutableInstructionDataEncoder(),
    getFailsDuplicateMutableInstructionDataDecoder(),
  );
}

/// Creates a [FailsDuplicateMutable] instruction.
Instruction getFailsDuplicateMutableInstruction({
  required Address programAddress,
  required Address account1,
  required Address account2,
}) {
  final instructionData = FailsDuplicateMutableInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: account1, role: AccountRole.writable),
      AccountMeta(address: account2, role: AccountRole.writable),
    ],
    data: getFailsDuplicateMutableInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [FailsDuplicateMutable] instruction from raw instruction data.
FailsDuplicateMutableInstructionData parseFailsDuplicateMutableInstruction(
  Instruction instruction,
) {
  return getFailsDuplicateMutableInstructionDataDecoder().decode(
    instruction.data!,
  );
}
