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
class InspectInstructionData {
  const InspectInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<InspectInstructionData> getInspectInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (InspectInstructionData value) => <String, Object?>{'discriminator': 2},
  );
}

Decoder<InspectInstructionData> getInspectInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'inspect instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InspectInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (InspectInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InspectInstructionData>(
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
      VariableSizeDecoder<InspectInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InspectInstructionData, InspectInstructionData>
getInspectInstructionDataCodec() {
  return combineCodec(
    getInspectInstructionDataEncoder(),
    getInspectInstructionDataDecoder(),
  );
}

/// Creates a [Inspect] instruction.
Instruction getInspectInstruction({
  required Address programAddress,
  required Address authority,
  Address? store,
  Address? witness,
}) {
  final instructionData = InspectInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      if (store != null)
        AccountMeta(address: store, role: AccountRole.readonly)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (witness != null)
        AccountMeta(address: witness, role: AccountRole.readonlySigner)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
    ],
    data: getInspectInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Inspect] instruction from raw instruction data.
InspectInstructionData parseInspectInstruction(Instruction instruction) {
  return getInspectInstructionDataDecoder().decode(instruction.data!);
}
