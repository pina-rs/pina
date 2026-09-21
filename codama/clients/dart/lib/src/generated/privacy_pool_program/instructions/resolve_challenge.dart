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
class ResolveChallengeInstructionData {
  const ResolveChallengeInstructionData({
    required this.approve,
  }) :
      discriminator = 10,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int approve;
}

Encoder<ResolveChallengeInstructionData> getResolveChallengeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('approve', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ResolveChallengeInstructionData value) => <String, Object?>{
      'discriminator': 10,
      'migrationVersion': 0,
      'approve': value.approve,
    },
  );
}

Decoder<ResolveChallengeInstructionData> getResolveChallengeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('approve', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'resolveChallenge instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (ResolveChallengeInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(10),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ResolveChallengeInstructionData(
      approve: map['approve']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ResolveChallengeInstructionData>(
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
      VariableSizeDecoder<ResolveChallengeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ResolveChallengeInstructionData, ResolveChallengeInstructionData> getResolveChallengeInstructionDataCodec() {
  return combineCodec(getResolveChallengeInstructionDataEncoder(), getResolveChallengeInstructionDataDecoder());
}

/// Creates a [ResolveChallenge] instruction.
Instruction getResolveChallengeInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required Address disclosureRequest,
  required int approve,
}) {
  final instructionData = ResolveChallengeInstructionData(
      approve: approve,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.readonlySigner),
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: disclosureRequest, role: AccountRole.writable),
    ],
    data: getResolveChallengeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ResolveChallenge] instruction from raw instruction data.
ResolveChallengeInstructionData parseResolveChallengeInstruction(Instruction instruction) {
  return getResolveChallengeInstructionDataDecoder().decode(instruction.data!);
}
