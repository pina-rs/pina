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
class UpdateRoleInstructionData {
  const UpdateRoleInstructionData({required this.permissions})
    : discriminator = 2;

  final int discriminator;
  final BigInt permissions;
}

Encoder<UpdateRoleInstructionData> getUpdateRoleInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('permissions', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateRoleInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'permissions': value.permissions,
    },
  );
}

Decoder<UpdateRoleInstructionData> getUpdateRoleInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('permissions', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'updateRole instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (UpdateRoleInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      UpdateRoleInstructionData(permissions: map['permissions']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<UpdateRoleInstructionData>(
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
      VariableSizeDecoder<UpdateRoleInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<UpdateRoleInstructionData, UpdateRoleInstructionData>
getUpdateRoleInstructionDataCodec() {
  return combineCodec(
    getUpdateRoleInstructionDataEncoder(),
    getUpdateRoleInstructionDataDecoder(),
  );
}

/// Creates a [UpdateRole] instruction.
Instruction getUpdateRoleInstruction({
  required Address programAddress,
  required Address admin,
  required Address registryConfig,
  required Address roleEntry,
  required BigInt permissions,
}) {
  final instructionData = UpdateRoleInstructionData(permissions: permissions);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.readonlySigner),
      AccountMeta(address: registryConfig, role: AccountRole.readonly),
      AccountMeta(address: roleEntry, role: AccountRole.writable),
    ],
    data: getUpdateRoleInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [UpdateRole] instruction from raw instruction data.
UpdateRoleInstructionData parseUpdateRoleInstruction(Instruction instruction) {
  return getUpdateRoleInstructionDataDecoder().decode(instruction.data!);
}
