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
class SysvarsInstructionData {
  const SysvarsInstructionData() : discriminator = 0;

  final int discriminator;
}

Encoder<SysvarsInstructionData> getSysvarsInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (SysvarsInstructionData value) => <String, Object?>{'discriminator': 0},
  );
}

Decoder<SysvarsInstructionData> getSysvarsInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'sysvars instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (SysvarsInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (SysvarsInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SysvarsInstructionData>(
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
      VariableSizeDecoder<SysvarsInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SysvarsInstructionData, SysvarsInstructionData>
getSysvarsInstructionDataCodec() {
  return combineCodec(
    getSysvarsInstructionDataEncoder(),
    getSysvarsInstructionDataDecoder(),
  );
}

/// Creates a [Sysvars] instruction.
Instruction getSysvarsInstruction({
  required Address programAddress,
  required Address clock,
  required Address rent,
  required Address stakeHistory,
}) {
  final instructionData = SysvarsInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: clock, role: AccountRole.readonly),
      AccountMeta(address: rent, role: AccountRole.readonly),
      AccountMeta(address: stakeHistory, role: AccountRole.readonly),
    ],
    data: getSysvarsInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Sysvars] instruction from raw instruction data.
SysvarsInstructionData parseSysvarsInstruction(Instruction instruction) {
  return getSysvarsInstructionDataDecoder().decode(instruction.data!);
}
