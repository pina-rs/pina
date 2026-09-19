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
class AllocateInstructionData {
  const AllocateInstructionData({required this.value})
    : discriminator = 0,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final BigInt value;
}

Encoder<AllocateInstructionData> getAllocateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('value', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AllocateInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'migrationVersion': 0,
      'value': value.value,
    },
  );
}

Decoder<AllocateInstructionData> getAllocateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('value', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'allocate instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (AllocateInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (AllocateInstructionData(value: map['value']! as BigInt), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AllocateInstructionData>(
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
      VariableSizeDecoder<AllocateInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<AllocateInstructionData, AllocateInstructionData>
getAllocateInstructionDataCodec() {
  return combineCodec(
    getAllocateInstructionDataEncoder(),
    getAllocateInstructionDataDecoder(),
  );
}

/// Creates a [Allocate] instruction.
Instruction getAllocateInstruction({
  required Address programAddress,

  required BigInt value,
}) {
  final instructionData = AllocateInstructionData(value: value);

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getAllocateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Allocate] instruction from raw instruction data.
AllocateInstructionData parseAllocateInstruction(Instruction instruction) {
  return getAllocateInstructionDataDecoder().decode(instruction.data!);
}
