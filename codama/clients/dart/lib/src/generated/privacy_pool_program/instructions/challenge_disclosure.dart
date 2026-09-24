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
class ChallengeDisclosureInstructionData {
  const ChallengeDisclosureInstructionData({required this.reserved})
    : discriminator = 9,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int reserved;
}

Encoder<ChallengeDisclosureInstructionData>
getChallengeDisclosureInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('reserved', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ChallengeDisclosureInstructionData value) => <String, Object?>{
      'discriminator': 9,
      'migrationVersion': 0,
      'reserved': value.reserved,
    },
  );
}

Decoder<ChallengeDisclosureInstructionData>
getChallengeDisclosureInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('reserved', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'challengeDisclosure instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ChallengeDisclosureInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(9)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ChallengeDisclosureInstructionData(reserved: map['reserved']! as int),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ChallengeDisclosureInstructionData>(
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
      VariableSizeDecoder<ChallengeDisclosureInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ChallengeDisclosureInstructionData, ChallengeDisclosureInstructionData>
getChallengeDisclosureInstructionDataCodec() {
  return combineCodec(
    getChallengeDisclosureInstructionDataEncoder(),
    getChallengeDisclosureInstructionDataDecoder(),
  );
}

/// Creates a [ChallengeDisclosure] instruction.
Instruction getChallengeDisclosureInstruction({
  required Address programAddress,
  required Address disclosureRequest,
  required Address noteCommitment,
  required Address viewer,
  required Address clock,
  required int reserved,
}) {
  final instructionData = ChallengeDisclosureInstructionData(
    reserved: reserved,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: disclosureRequest, role: AccountRole.writable),
      AccountMeta(address: noteCommitment, role: AccountRole.readonly),
      AccountMeta(address: viewer, role: AccountRole.readonly),
      AccountMeta(address: clock, role: AccountRole.readonly),
    ],
    data: getChallengeDisclosureInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [ChallengeDisclosure] instruction from raw instruction data.
ChallengeDisclosureInstructionData parseChallengeDisclosureInstruction(
  Instruction instruction,
) {
  return getChallengeDisclosureInstructionDataDecoder().decode(
    instruction.data!,
  );
}
