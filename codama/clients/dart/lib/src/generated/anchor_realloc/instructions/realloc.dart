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
class ReallocInstructionData {
  const ReallocInstructionData({required this.len}) : discriminator = 0;

  final int discriminator;
  final int len;
}

Encoder<ReallocInstructionData> getReallocInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('len', getU16Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ReallocInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'len': value.len,
    },
  );
}

Decoder<ReallocInstructionData> getReallocInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('len', getU16Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'realloc instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ReallocInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ReallocInstructionData(len: map['len']! as int), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ReallocInstructionData>(
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
      VariableSizeDecoder<ReallocInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ReallocInstructionData, ReallocInstructionData>
getReallocInstructionDataCodec() {
  return combineCodec(
    getReallocInstructionDataEncoder(),
    getReallocInstructionDataDecoder(),
  );
}

/// Creates a [Realloc] instruction.
Instruction getReallocInstruction({
  required Address programAddress,
  required Address authority,
  required Address sample,
  required Address systemProgram,
  required int len,
}) {
  final instructionData = ReallocInstructionData(len: len);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: sample, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getReallocInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Realloc] instruction from raw instruction data.
ReallocInstructionData parseReallocInstruction(Instruction instruction) {
  return getReallocInstructionDataDecoder().decode(instruction.data!);
}
