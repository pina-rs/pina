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
class CreateInstructionData {
  const CreateInstructionData({required this.dataF32, required this.dataF64})
    : discriminator = 0;

  final int discriminator;
  final int dataF32;
  final BigInt dataF64;
}

Encoder<CreateInstructionData> getCreateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('dataF32', getU32Encoder()),
    ('dataF64', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CreateInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'dataF32': value.dataF32,
      'dataF64': value.dataF64,
    },
  );
}

Decoder<CreateInstructionData> getCreateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('dataF32', getU32Decoder()),
    ('dataF64', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'create instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CreateInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      CreateInstructionData(
        dataF32: map['dataF32']! as int,
        dataF64: map['dataF64']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CreateInstructionData>(
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
      VariableSizeDecoder<CreateInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CreateInstructionData, CreateInstructionData>
getCreateInstructionDataCodec() {
  return combineCodec(
    getCreateInstructionDataEncoder(),
    getCreateInstructionDataDecoder(),
  );
}

/// Creates a [Create] instruction.
Instruction getCreateInstruction({
  required Address programAddress,
  required Address account,
  required Address authority,
  required Address systemProgram,
  required int dataF32,
  required BigInt dataF64,
}) {
  final instructionData = CreateInstructionData(
    dataF32: dataF32,
    dataF64: dataF64,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: account, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getCreateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Create] instruction from raw instruction data.
CreateInstructionData parseCreateInstruction(Instruction instruction) {
  return getCreateInstructionDataDecoder().decode(instruction.data!);
}
