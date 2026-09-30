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
class RemoveTagInstructionData {
  const RemoveTagInstructionData({required this.index}) : discriminator = 3;

  final int discriminator;
  final BigInt index;
}

Encoder<RemoveTagInstructionData> getRemoveTagInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('index', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RemoveTagInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'index': value.index,
    },
  );
}

Decoder<RemoveTagInstructionData> getRemoveTagInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('index', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'removeTag instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RemoveTagInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RemoveTagInstructionData(index: map['index']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RemoveTagInstructionData>(
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
      VariableSizeDecoder<RemoveTagInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RemoveTagInstructionData, RemoveTagInstructionData>
getRemoveTagInstructionDataCodec() {
  return combineCodec(
    getRemoveTagInstructionDataEncoder(),
    getRemoveTagInstructionDataDecoder(),
  );
}

/// Creates a [RemoveTag] instruction.
Instruction getRemoveTagInstruction({
  required Address programAddress,
  required Address authority,
  required Address profile,
  required BigInt index,
}) {
  final instructionData = RemoveTagInstructionData(index: index);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: profile, role: AccountRole.writable),
    ],
    data: getRemoveTagInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RemoveTag] instruction from raw instruction data.
RemoveTagInstructionData parseRemoveTagInstruction(Instruction instruction) {
  return getRemoveTagInstructionDataDecoder().decode(instruction.data!);
}
