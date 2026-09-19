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
class ConfigInitializeInstructionData {
  const ConfigInitializeInstructionData({
    required this.bump,
    required this.treasury,
    required this.creationFee,
  }) : discriminator = 0,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address treasury;
  final BigInt creationFee;
}

Encoder<ConfigInitializeInstructionData>
getConfigInitializeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('treasury', getAddressEncoder()),
    ('creationFee', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ConfigInitializeInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'migrationVersion': 0,
      'bump': value.bump,
      'treasury': value.treasury,
      'creationFee': value.creationFee,
    },
  );
}

Decoder<ConfigInitializeInstructionData>
getConfigInitializeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('treasury', getAddressDecoder()),
    ('creationFee', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'configInitialize instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ConfigInitializeInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ConfigInitializeInstructionData(
        bump: map['bump']! as int,
        treasury: map['treasury']! as Address,
        creationFee: map['creationFee']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ConfigInitializeInstructionData>(
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
      VariableSizeDecoder<ConfigInitializeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ConfigInitializeInstructionData, ConfigInitializeInstructionData>
getConfigInitializeInstructionDataCodec() {
  return combineCodec(
    getConfigInitializeInstructionDataEncoder(),
    getConfigInitializeInstructionDataDecoder(),
  );
}

/// Creates a [ConfigInitialize] instruction.
Instruction getConfigInitializeInstruction({
  required Address programAddress,
  required Address authority,
  required Address programConfig,
  required Address systemProgram,
  required int bump,
  required Address treasury,
  required BigInt creationFee,
}) {
  final instructionData = ConfigInitializeInstructionData(
    bump: bump,
    treasury: treasury,
    creationFee: creationFee,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: programConfig, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getConfigInitializeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ConfigInitialize] instruction from raw instruction data.
ConfigInitializeInstructionData parseConfigInitializeInstruction(
  Instruction instruction,
) {
  return getConfigInitializeInstructionDataDecoder().decode(instruction.data!);
}
