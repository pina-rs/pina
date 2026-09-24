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
class ProposalApproveInstructionData {
  const ProposalApproveInstructionData() :
      discriminator = 6,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
}

Encoder<ProposalApproveInstructionData> getProposalApproveInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ProposalApproveInstructionData value) => <String, Object?>{
      'discriminator': 6,
      'migrationVersion': 0,
    },
  );
}

Decoder<ProposalApproveInstructionData> getProposalApproveInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'proposalApprove instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (ProposalApproveInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(6),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ProposalApproveInstructionData(

      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ProposalApproveInstructionData>(
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
      VariableSizeDecoder<ProposalApproveInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ProposalApproveInstructionData, ProposalApproveInstructionData> getProposalApproveInstructionDataCodec() {
  return combineCodec(getProposalApproveInstructionDataEncoder(), getProposalApproveInstructionDataDecoder());
}

/// Creates a [ProposalApprove] instruction.
Instruction getProposalApproveInstruction({
  required Address programAddress,
  required Address multisig,
  required Address proposal,
  required Address member,
  required Address clock,

}) {
  final instructionData = ProposalApproveInstructionData(

  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: multisig, role: AccountRole.readonly),
    AccountMeta(address: proposal, role: AccountRole.writable),
    AccountMeta(address: member, role: AccountRole.readonlySigner),
    AccountMeta(address: clock, role: AccountRole.readonly),
    ],
    data: getProposalApproveInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ProposalApprove] instruction from raw instruction data.
ProposalApproveInstructionData parseProposalApproveInstruction(Instruction instruction) {
  return getProposalApproveInstructionDataDecoder().decode(instruction.data!);
}
