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
class ApproveDisclosureInstructionData {
  const ApproveDisclosureInstructionData({
    required this.reserved,
  }) :
      discriminator = 11,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int reserved;
}

Encoder<ApproveDisclosureInstructionData> getApproveDisclosureInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('reserved', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ApproveDisclosureInstructionData value) => <String, Object?>{
      'discriminator': 11,
      'migrationVersion': 0,
      'reserved': value.reserved,
    },
  );
}

Decoder<ApproveDisclosureInstructionData> getApproveDisclosureInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('reserved', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'approveDisclosure instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (ApproveDisclosureInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(11),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ApproveDisclosureInstructionData(
      reserved: map['reserved']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ApproveDisclosureInstructionData>(
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
      VariableSizeDecoder<ApproveDisclosureInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ApproveDisclosureInstructionData, ApproveDisclosureInstructionData> getApproveDisclosureInstructionDataCodec() {
  return combineCodec(getApproveDisclosureInstructionDataEncoder(), getApproveDisclosureInstructionDataDecoder());
}

/// Creates a [ApproveDisclosure] instruction.
Instruction getApproveDisclosureInstruction({
  required Address programAddress,
  required Address custodian,
  required Address poolConfig,
  required Address custodianRegistry,
  required Address disclosureRequest,
  required Address disclosureLog,
  required Address clock,
  required int reserved,
}) {
  final instructionData = ApproveDisclosureInstructionData(
      reserved: reserved,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: custodian, role: AccountRole.readonlySigner),
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: custodianRegistry, role: AccountRole.readonly),
    AccountMeta(address: disclosureRequest, role: AccountRole.writable),
    AccountMeta(address: disclosureLog, role: AccountRole.writable),
    AccountMeta(address: clock, role: AccountRole.readonly),
    ],
    data: getApproveDisclosureInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ApproveDisclosure] instruction from raw instruction data.
ApproveDisclosureInstructionData parseApproveDisclosureInstruction(Instruction instruction) {
  return getApproveDisclosureInstructionDataDecoder().decode(instruction.data!);
}
