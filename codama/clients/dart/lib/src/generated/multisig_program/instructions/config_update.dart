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
class ConfigUpdateInstructionData {
  const ConfigUpdateInstructionData({
    required this.setTreasury,
    required this.treasury,
    required this.setCreationFee,
    required this.creationFee,
  }) : discriminator = 1,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final bool setTreasury;
  final Address treasury;
  final bool setCreationFee;
  final BigInt creationFee;
}

Encoder<ConfigUpdateInstructionData> getConfigUpdateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('setTreasury', getBooleanEncoder()),
    ('treasury', getAddressEncoder()),
    ('setCreationFee', getBooleanEncoder()),
    ('creationFee', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ConfigUpdateInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'setTreasury': value.setTreasury,
      'treasury': value.treasury,
      'setCreationFee': value.setCreationFee,
      'creationFee': value.creationFee,
    },
  );
}

Decoder<ConfigUpdateInstructionData> getConfigUpdateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('setTreasury', getBooleanDecoder()),
    ('treasury', getAddressDecoder()),
    ('setCreationFee', getBooleanDecoder()),
    ('creationFee', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'configUpdate instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ConfigUpdateInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ConfigUpdateInstructionData(
        setTreasury: map['setTreasury']! as bool,
        treasury: map['treasury']! as Address,
        setCreationFee: map['setCreationFee']! as bool,
        creationFee: map['creationFee']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ConfigUpdateInstructionData>(
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
      VariableSizeDecoder<ConfigUpdateInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ConfigUpdateInstructionData, ConfigUpdateInstructionData>
getConfigUpdateInstructionDataCodec() {
  return combineCodec(
    getConfigUpdateInstructionDataEncoder(),
    getConfigUpdateInstructionDataDecoder(),
  );
}

/// Creates a [ConfigUpdate] instruction.
Instruction getConfigUpdateInstruction({
  required Address programAddress,
  required Address authority,
  required Address programConfig,
  required bool setTreasury,
  required Address treasury,
  required bool setCreationFee,
  required BigInt creationFee,
}) {
  final instructionData = ConfigUpdateInstructionData(
    setTreasury: setTreasury,
    treasury: treasury,
    setCreationFee: setCreationFee,
    creationFee: creationFee,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: programConfig, role: AccountRole.writable),
    ],
    data: getConfigUpdateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ConfigUpdate] instruction from raw instruction data.
ConfigUpdateInstructionData parseConfigUpdateInstruction(
  Instruction instruction,
) {
  return getConfigUpdateInstructionDataDecoder().decode(instruction.data!);
}
