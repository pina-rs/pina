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
class RotateAdminInstructionData {
  const RotateAdminInstructionData() : discriminator = 4;

  final int discriminator;
}

Encoder<RotateAdminInstructionData> getRotateAdminInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RotateAdminInstructionData value) => <String, Object?>{'discriminator': 4},
  );
}

Decoder<RotateAdminInstructionData> getRotateAdminInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'rotateAdmin instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RotateAdminInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(4)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (RotateAdminInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RotateAdminInstructionData>(
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
      VariableSizeDecoder<RotateAdminInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RotateAdminInstructionData, RotateAdminInstructionData>
getRotateAdminInstructionDataCodec() {
  return combineCodec(
    getRotateAdminInstructionDataEncoder(),
    getRotateAdminInstructionDataDecoder(),
  );
}

/// Creates a [RotateAdmin] instruction.
Instruction getRotateAdminInstruction({
  required Address programAddress,
  required Address admin,
  required Address newAdmin,
  required Address registryConfig,
}) {
  final instructionData = RotateAdminInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.readonlySigner),
      AccountMeta(address: newAdmin, role: AccountRole.readonly),
      AccountMeta(address: registryConfig, role: AccountRole.writable),
    ],
    data: getRotateAdminInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RotateAdmin] instruction from raw instruction data.
RotateAdminInstructionData parseRotateAdminInstruction(
  Instruction instruction,
) {
  return getRotateAdminInstructionDataDecoder().decode(instruction.data!);
}
