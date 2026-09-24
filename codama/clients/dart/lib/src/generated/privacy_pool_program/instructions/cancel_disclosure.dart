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
class CancelDisclosureInstructionData {
  const CancelDisclosureInstructionData({required this.reserved})
    : discriminator = 12,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int reserved;
}

Encoder<CancelDisclosureInstructionData>
getCancelDisclosureInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('reserved', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CancelDisclosureInstructionData value) => <String, Object?>{
      'discriminator': 12,
      'migrationVersion': 0,
      'reserved': value.reserved,
    },
  );
}

Decoder<CancelDisclosureInstructionData>
getCancelDisclosureInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('reserved', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'cancelDisclosure instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CancelDisclosureInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(12)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      CancelDisclosureInstructionData(reserved: map['reserved']! as int),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CancelDisclosureInstructionData>(
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
      VariableSizeDecoder<CancelDisclosureInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CancelDisclosureInstructionData, CancelDisclosureInstructionData>
getCancelDisclosureInstructionDataCodec() {
  return combineCodec(
    getCancelDisclosureInstructionDataEncoder(),
    getCancelDisclosureInstructionDataDecoder(),
  );
}

/// Creates a [CancelDisclosure] instruction.
Instruction getCancelDisclosureInstruction({
  required Address programAddress,
  required Address requester,
  required Address disclosureRequest,
  required int reserved,
}) {
  final instructionData = CancelDisclosureInstructionData(reserved: reserved);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: requester, role: AccountRole.readonlySigner),
      AccountMeta(address: disclosureRequest, role: AccountRole.writable),
    ],
    data: getCancelDisclosureInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [CancelDisclosure] instruction from raw instruction data.
CancelDisclosureInstructionData parseCancelDisclosureInstruction(
  Instruction instruction,
) {
  return getCancelDisclosureInstructionDataDecoder().decode(instruction.data!);
}
