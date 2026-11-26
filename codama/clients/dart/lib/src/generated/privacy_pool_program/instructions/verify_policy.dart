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
class VerifyPolicyInstructionData {
  const VerifyPolicyInstructionData({
    required this.policyHash,
    required this.proofA,
    required this.proofB,
    required this.proofC,
  }) : discriminator = 17;

  final int discriminator;
  final Uint8List policyHash;
  final Uint8List proofA;
  final Uint8List proofB;
  final Uint8List proofC;
}

Encoder<VerifyPolicyInstructionData> getVerifyPolicyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    (
      'policyHash',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('proofA', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('proofB', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('proofC', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (VerifyPolicyInstructionData value) => <String, Object?>{
      'discriminator': 17,
      'policyHash': value.policyHash,
      'proofA': value.proofA,
      'proofB': value.proofB,
      'proofC': value.proofC,
    },
  );
}

Decoder<VerifyPolicyInstructionData> getVerifyPolicyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('policyHash', fixDecoderSize(getBytesDecoder(), 32)),
    ('proofA', fixDecoderSize(getBytesDecoder(), 32)),
    ('proofB', fixDecoderSize(getBytesDecoder(), 128)),
    ('proofC', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'verifyPolicy instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (VerifyPolicyInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(17)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      VerifyPolicyInstructionData(
        policyHash: map['policyHash']! as Uint8List,
        proofA: map['proofA']! as Uint8List,
        proofB: map['proofB']! as Uint8List,
        proofC: map['proofC']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<VerifyPolicyInstructionData>(
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
      VariableSizeDecoder<VerifyPolicyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<VerifyPolicyInstructionData, VerifyPolicyInstructionData>
getVerifyPolicyInstructionDataCodec() {
  return combineCodec(
    getVerifyPolicyInstructionDataEncoder(),
    getVerifyPolicyInstructionDataDecoder(),
  );
}

/// Creates a [VerifyPolicy] instruction.
Instruction getVerifyPolicyInstruction({
  required Address programAddress,
  required Address verifyingKeyAccount,
  required Uint8List policyHash,
  required Uint8List proofA,
  required Uint8List proofB,
  required Uint8List proofC,
}) {
  final instructionData = VerifyPolicyInstructionData(
    policyHash: policyHash,
    proofA: proofA,
    proofB: proofB,
    proofC: proofC,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: verifyingKeyAccount, role: AccountRole.readonly),
    ],
    data: getVerifyPolicyInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [VerifyPolicy] instruction from raw instruction data.
VerifyPolicyInstructionData parseVerifyPolicyInstruction(
  Instruction instruction,
) {
  return getVerifyPolicyInstructionDataDecoder().decode(instruction.data!);
}
