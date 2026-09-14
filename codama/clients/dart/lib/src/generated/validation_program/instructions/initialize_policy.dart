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
class InitializePolicyInstructionData {
  const InitializePolicyInstructionData({
    required this.bump,
    required this.minimum,
    required this.maximum,
    required this.requiredApprovals,
  }) :
      discriminator = 0;

  final int discriminator;
  final int bump;
  final BigInt minimum;
  final BigInt maximum;
  final int requiredApprovals;
}

Encoder<InitializePolicyInstructionData> getInitializePolicyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('minimum', getU64Encoder()),
    ('maximum', getU64Encoder()),
    ('requiredApprovals', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializePolicyInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'bump': value.bump,
      'minimum': value.minimum,
      'maximum': value.maximum,
      'requiredApprovals': value.requiredApprovals,
    },
  );
}

Decoder<InitializePolicyInstructionData> getInitializePolicyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('minimum', getU64Decoder()),
    ('maximum', getU64Decoder()),
    ('requiredApprovals', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'initializePolicy instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (InitializePolicyInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      InitializePolicyInstructionData(
      bump: map['bump']! as int,
      minimum: map['minimum']! as BigInt,
      maximum: map['maximum']! as BigInt,
      requiredApprovals: map['requiredApprovals']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitializePolicyInstructionData>(
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
      VariableSizeDecoder<InitializePolicyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitializePolicyInstructionData, InitializePolicyInstructionData> getInitializePolicyInstructionDataCodec() {
  return combineCodec(getInitializePolicyInstructionDataEncoder(), getInitializePolicyInstructionDataDecoder());
}

/// Creates a [InitializePolicy] instruction.
Instruction getInitializePolicyInstruction({
  required Address programAddress,
  required Address authority,
  required Address policy,
  required Address systemProgram,
  required int bump,
  required BigInt minimum,
  required BigInt maximum,
  required int requiredApprovals,
}) {
  final instructionData = InitializePolicyInstructionData(
      bump: bump,
      minimum: minimum,
      maximum: maximum,
      requiredApprovals: requiredApprovals,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.writableSigner),
    AccountMeta(address: policy, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getInitializePolicyInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [InitializePolicy] instruction from raw instruction data.
InitializePolicyInstructionData parseInitializePolicyInstruction(Instruction instruction) {
  return getInitializePolicyInstructionDataDecoder().decode(instruction.data!);
}
