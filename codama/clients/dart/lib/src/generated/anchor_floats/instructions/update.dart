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
class UpdateInstructionData {
  const UpdateInstructionData({required this.dataF32, required this.dataF64})
    : discriminator = 1;

  final int discriminator;
  final int dataF32;
  final BigInt dataF64;
}

Encoder<UpdateInstructionData> getUpdateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('dataF32', getU32Encoder()),
    ('dataF64', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'dataF32': value.dataF32,
      'dataF64': value.dataF64,
    },
  );
}

Decoder<UpdateInstructionData> getUpdateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('dataF32', getU32Decoder()),
    ('dataF64', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'update instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (UpdateInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      UpdateInstructionData(
        dataF32: map['dataF32']! as int,
        dataF64: map['dataF64']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<UpdateInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readExact(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<UpdateInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<UpdateInstructionData, UpdateInstructionData>
getUpdateInstructionDataCodec() {
  return combineCodec(
    getUpdateInstructionDataEncoder(),
    getUpdateInstructionDataDecoder(),
  );
}

/// Creates a [Update] instruction.
Instruction getUpdateInstruction({
  required Address programAddress,
  required Address account,
  required Address authority,
  required int dataF32,
  required BigInt dataF64,
}) {
  final instructionData = UpdateInstructionData(
    dataF32: dataF32,
    dataF64: dataF64,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: account, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
    ],
    data: getUpdateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Update] instruction from raw instruction data.
UpdateInstructionData parseUpdateInstruction(Instruction instruction) {
  return getUpdateInstructionDataDecoder().decode(instruction.data!);
}
