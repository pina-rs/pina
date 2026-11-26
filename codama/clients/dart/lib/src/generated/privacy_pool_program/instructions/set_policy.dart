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
class SetPolicyInstructionData {
  const SetPolicyInstructionData({
    required this.policyFlags,
    required this.policyCosigner,
  }) :
      discriminator = 16;

  final int discriminator;
  final int policyFlags;
  final Address policyCosigner;
}

Encoder<SetPolicyInstructionData> getSetPolicyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('policyFlags', getU8Encoder()),
    ('policyCosigner', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (SetPolicyInstructionData value) => <String, Object?>{
      'discriminator': 16,
      'policyFlags': value.policyFlags,
      'policyCosigner': value.policyCosigner,
    },
  );
}

Decoder<SetPolicyInstructionData> getSetPolicyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('policyFlags', getU8Decoder()),
    ('policyCosigner', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'setPolicy instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (SetPolicyInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(16),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      SetPolicyInstructionData(
      policyFlags: map['policyFlags']! as int,
      policyCosigner: map['policyCosigner']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SetPolicyInstructionData>(
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
      VariableSizeDecoder<SetPolicyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SetPolicyInstructionData, SetPolicyInstructionData> getSetPolicyInstructionDataCodec() {
  return combineCodec(getSetPolicyInstructionDataEncoder(), getSetPolicyInstructionDataDecoder());
}

/// Creates a [SetPolicy] instruction.
Instruction getSetPolicyInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required int policyFlags,
  required Address policyCosigner,
}) {
  final instructionData = SetPolicyInstructionData(
      policyFlags: policyFlags,
      policyCosigner: policyCosigner,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.readonlySigner),
    AccountMeta(address: poolConfig, role: AccountRole.writable),
    ],
    data: getSetPolicyInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [SetPolicy] instruction from raw instruction data.
SetPolicyInstructionData parseSetPolicyInstruction(Instruction instruction) {
  return getSetPolicyInstructionDataDecoder().decode(instruction.data!);
}
