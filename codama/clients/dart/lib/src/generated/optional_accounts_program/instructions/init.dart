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
class InitInstructionData {
  const InitInstructionData({required this.bump}) : discriminator = 0;

  final int discriminator;
  final int bump;
}

Encoder<InitInstructionData> getInitInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (InitInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'bump': value.bump,
    },
  );
}

Decoder<InitInstructionData> getInitInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'init instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InitInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (InitInstructionData(bump: map['bump']! as int), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitInstructionData>(
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
      VariableSizeDecoder<InitInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitInstructionData, InitInstructionData> getInitInstructionDataCodec() {
  return combineCodec(
    getInitInstructionDataEncoder(),
    getInitInstructionDataDecoder(),
  );
}

/// Creates a [Init] instruction.
Instruction getInitInstruction({
  required Address programAddress,
  required Address authority,
  required Address store,
  required Address systemProgram,
  required int bump,
}) {
  final instructionData = InitInstructionData(bump: bump);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: store, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getInitInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Init] instruction from raw instruction data.
InitInstructionData parseInitInstruction(Instruction instruction) {
  return getInitInstructionDataDecoder().decode(instruction.data!);
}
