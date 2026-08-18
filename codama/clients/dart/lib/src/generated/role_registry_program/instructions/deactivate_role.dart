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
class DeactivateRoleInstructionData {
  const DeactivateRoleInstructionData() : discriminator = 3;

  final int discriminator;
}

Encoder<DeactivateRoleInstructionData>
getDeactivateRoleInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (DeactivateRoleInstructionData value) => <String, Object?>{
      'discriminator': 3,
    },
  );
}

Decoder<DeactivateRoleInstructionData>
getDeactivateRoleInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'deactivateRole instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (DeactivateRoleInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (DeactivateRoleInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<DeactivateRoleInstructionData>(
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
      VariableSizeDecoder<DeactivateRoleInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DeactivateRoleInstructionData, DeactivateRoleInstructionData>
getDeactivateRoleInstructionDataCodec() {
  return combineCodec(
    getDeactivateRoleInstructionDataEncoder(),
    getDeactivateRoleInstructionDataDecoder(),
  );
}

/// Creates a [DeactivateRole] instruction.
Instruction getDeactivateRoleInstruction({
  required Address programAddress,
  required Address admin,
  required Address registryConfig,
  required Address roleEntry,
}) {
  final instructionData = DeactivateRoleInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.readonlySigner),
      AccountMeta(address: registryConfig, role: AccountRole.readonly),
      AccountMeta(address: roleEntry, role: AccountRole.writable),
    ],
    data: getDeactivateRoleInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [DeactivateRole] instruction from raw instruction data.
DeactivateRoleInstructionData parseDeactivateRoleInstruction(
  Instruction instruction,
) {
  return getDeactivateRoleInstructionDataDecoder().decode(instruction.data!);
}
