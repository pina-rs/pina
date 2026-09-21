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
class ProposalCloseInstructionData {
  const ProposalCloseInstructionData()
    : discriminator = 14,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
}

Encoder<ProposalCloseInstructionData> getProposalCloseInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ProposalCloseInstructionData value) => <String, Object?>{
      'discriminator': 14,
      'migrationVersion': 0,
    },
  );
}

Decoder<ProposalCloseInstructionData> getProposalCloseInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'proposalClose instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ProposalCloseInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(14)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ProposalCloseInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ProposalCloseInstructionData>(
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
      VariableSizeDecoder<ProposalCloseInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ProposalCloseInstructionData, ProposalCloseInstructionData>
getProposalCloseInstructionDataCodec() {
  return combineCodec(
    getProposalCloseInstructionDataEncoder(),
    getProposalCloseInstructionDataDecoder(),
  );
}

/// Creates a [ProposalClose] instruction.
Instruction getProposalCloseInstruction({
  required Address programAddress,
  required Address multisig,
  required Address proposal,
  required Address rentCollector,
}) {
  final instructionData = ProposalCloseInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: multisig, role: AccountRole.readonly),
      AccountMeta(address: proposal, role: AccountRole.writable),
      AccountMeta(address: rentCollector, role: AccountRole.writable),
    ],
    data: getProposalCloseInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ProposalClose] instruction from raw instruction data.
ProposalCloseInstructionData parseProposalCloseInstruction(
  Instruction instruction,
) {
  return getProposalCloseInstructionDataDecoder().decode(instruction.data!);
}
