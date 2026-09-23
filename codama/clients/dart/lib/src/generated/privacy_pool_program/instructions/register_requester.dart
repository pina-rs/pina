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
class RegisterRequesterInstructionData {
  const RegisterRequesterInstructionData({
    required this.requester,
    required this.maxTier,
  }) : discriminator = 3,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Address requester;
  final int maxTier;
}

Encoder<RegisterRequesterInstructionData>
getRegisterRequesterInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('requester', getAddressEncoder()),
    ('maxTier', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RegisterRequesterInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'migrationVersion': 0,
      'requester': value.requester,
      'maxTier': value.maxTier,
    },
  );
}

Decoder<RegisterRequesterInstructionData>
getRegisterRequesterInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('requester', getAddressDecoder()),
    ('maxTier', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'registerRequester instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RegisterRequesterInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RegisterRequesterInstructionData(
        requester: map['requester']! as Address,
        maxTier: map['maxTier']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RegisterRequesterInstructionData>(
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
      VariableSizeDecoder<RegisterRequesterInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RegisterRequesterInstructionData, RegisterRequesterInstructionData>
getRegisterRequesterInstructionDataCodec() {
  return combineCodec(
    getRegisterRequesterInstructionDataEncoder(),
    getRegisterRequesterInstructionDataDecoder(),
  );
}

/// Creates a [RegisterRequester] instruction.
Instruction getRegisterRequesterInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required Address requesterRegistry,
  required Address requester,
  required int maxTier,
}) {
  final instructionData = RegisterRequesterInstructionData(
    requester: requester,
    maxTier: maxTier,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: poolConfig, role: AccountRole.readonly),
      AccountMeta(address: requesterRegistry, role: AccountRole.writable),
    ],
    data: getRegisterRequesterInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RegisterRequester] instruction from raw instruction data.
RegisterRequesterInstructionData parseRegisterRequesterInstruction(
  Instruction instruction,
) {
  return getRegisterRequesterInstructionDataDecoder().decode(instruction.data!);
}
