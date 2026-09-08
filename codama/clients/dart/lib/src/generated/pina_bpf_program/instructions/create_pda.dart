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
class CreatePdaInstructionData {
  const CreatePdaInstructionData({required this.bump}) : discriminator = 3;

  final int discriminator;
  final int bump;
}

Encoder<CreatePdaInstructionData> getCreatePdaInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CreatePdaInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'bump': value.bump,
    },
  );
}

Decoder<CreatePdaInstructionData> getCreatePdaInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'createPda instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CreatePdaInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (CreatePdaInstructionData(bump: map['bump']! as int), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CreatePdaInstructionData>(
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
      VariableSizeDecoder<CreatePdaInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CreatePdaInstructionData, CreatePdaInstructionData>
getCreatePdaInstructionDataCodec() {
  return combineCodec(
    getCreatePdaInstructionDataEncoder(),
    getCreatePdaInstructionDataDecoder(),
  );
}

/// Creates a [CreatePda] instruction.
Instruction getCreatePdaInstruction({
  required Address programAddress,
  required Address payer,
  required Address state,
  required Address systemProgram,
  required int bump,
}) {
  final instructionData = CreatePdaInstructionData(bump: bump);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: payer, role: AccountRole.writableSigner),
      AccountMeta(address: state, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getCreatePdaInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [CreatePda] instruction from raw instruction data.
CreatePdaInstructionData parseCreatePdaInstruction(Instruction instruction) {
  return getCreatePdaInstructionDataDecoder().decode(instruction.data!);
}
