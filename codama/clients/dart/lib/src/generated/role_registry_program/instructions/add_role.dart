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
class AddRoleInstructionData {
  const AddRoleInstructionData({
    required this.roleId,
    required this.permissions,
    required this.bump,
  }) : discriminator = 1;

  final int discriminator;
  final BigInt roleId;
  final BigInt permissions;
  final int bump;
}

Encoder<AddRoleInstructionData> getAddRoleInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('roleId', getU64Encoder()),
    ('permissions', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AddRoleInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'roleId': value.roleId,
      'permissions': value.permissions,
      'bump': value.bump,
    },
  );
}

Decoder<AddRoleInstructionData> getAddRoleInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('roleId', getU64Decoder()),
    ('permissions', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'addRole instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (AddRoleInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      AddRoleInstructionData(
        roleId: map['roleId']! as BigInt,
        permissions: map['permissions']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AddRoleInstructionData>(
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
      VariableSizeDecoder<AddRoleInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<AddRoleInstructionData, AddRoleInstructionData>
getAddRoleInstructionDataCodec() {
  return combineCodec(
    getAddRoleInstructionDataEncoder(),
    getAddRoleInstructionDataDecoder(),
  );
}

/// Creates a [AddRole] instruction.
Instruction getAddRoleInstruction({
  required Address programAddress,
  required Address admin,
  required Address grantee,
  required Address registryConfig,
  required Address roleEntry,
  required Address systemProgram,
  required BigInt roleId,
  required BigInt permissions,
  required int bump,
}) {
  final instructionData = AddRoleInstructionData(
    roleId: roleId,
    permissions: permissions,
    bump: bump,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.writableSigner),
      AccountMeta(address: grantee, role: AccountRole.readonly),
      AccountMeta(address: registryConfig, role: AccountRole.writable),
      AccountMeta(address: roleEntry, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getAddRoleInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [AddRole] instruction from raw instruction data.
AddRoleInstructionData parseAddRoleInstruction(Instruction instruction) {
  return getAddRoleInstructionDataDecoder().decode(instruction.data!);
}
