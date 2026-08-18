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
class AddTagInstructionData {
  const AddTagInstructionData({required this.tag}) : discriminator = 2;

  final int discriminator;
  final BigInt tag;
}

Encoder<AddTagInstructionData> getAddTagInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('tag', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AddTagInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'tag': value.tag,
    },
  );
}

Decoder<AddTagInstructionData> getAddTagInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('tag', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'addTag instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (AddTagInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (AddTagInstructionData(tag: map['tag']! as BigInt), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AddTagInstructionData>(
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
      VariableSizeDecoder<AddTagInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<AddTagInstructionData, AddTagInstructionData>
getAddTagInstructionDataCodec() {
  return combineCodec(
    getAddTagInstructionDataEncoder(),
    getAddTagInstructionDataDecoder(),
  );
}

/// Creates a [AddTag] instruction.
Instruction getAddTagInstruction({
  required Address programAddress,
  required Address authority,
  required Address profile,
  required BigInt tag,
}) {
  final instructionData = AddTagInstructionData(tag: tag);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: profile, role: AccountRole.writable),
    ],
    data: getAddTagInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [AddTag] instruction from raw instruction data.
AddTagInstructionData parseAddTagInstruction(Instruction instruction) {
  return getAddTagInstructionDataDecoder().decode(instruction.data!);
}
