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
class ValidateExternalProgramInstructionData {
  const ValidateExternalProgramInstructionData() : discriminator = 0;

  final int discriminator;
}

Encoder<ValidateExternalProgramInstructionData>
getValidateExternalProgramInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ValidateExternalProgramInstructionData value) => <String, Object?>{
      'discriminator': 0,
    },
  );
}

Decoder<ValidateExternalProgramInstructionData>
getValidateExternalProgramInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'validateExternalProgram instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ValidateExternalProgramInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ValidateExternalProgramInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ValidateExternalProgramInstructionData>(
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
      VariableSizeDecoder<ValidateExternalProgramInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<
  ValidateExternalProgramInstructionData,
  ValidateExternalProgramInstructionData
>
getValidateExternalProgramInstructionDataCodec() {
  return combineCodec(
    getValidateExternalProgramInstructionDataEncoder(),
    getValidateExternalProgramInstructionDataDecoder(),
  );
}

/// Creates a [ValidateExternalProgram] instruction.
Instruction getValidateExternalProgramInstruction({
  required Address programAddress,
  required Address authority,
  required Address externalProgram,
}) {
  final instructionData = ValidateExternalProgramInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: externalProgram, role: AccountRole.readonly),
    ],
    data: getValidateExternalProgramInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [ValidateExternalProgram] instruction from raw instruction data.
ValidateExternalProgramInstructionData parseValidateExternalProgramInstruction(
  Instruction instruction,
) {
  return getValidateExternalProgramInstructionDataDecoder().decode(
    instruction.data!,
  );
}
