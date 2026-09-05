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
class WriteInstructionData {
  const WriteInstructionData({required this.index, required this.value})
    : discriminator = 2;

  final int discriminator;
  final int index;
  final BigInt value;
}

Encoder<WriteInstructionData> getWriteInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('index', getU8Encoder()),
    ('value', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (WriteInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'index': value.index,
      'value': value.value,
    },
  );
}

Decoder<WriteInstructionData> getWriteInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('index', getU8Decoder()),
    ('value', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'write instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (WriteInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      WriteInstructionData(
        index: map['index']! as int,
        value: map['value']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<WriteInstructionData>(
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
      VariableSizeDecoder<WriteInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<WriteInstructionData, WriteInstructionData>
getWriteInstructionDataCodec() {
  return combineCodec(
    getWriteInstructionDataEncoder(),
    getWriteInstructionDataDecoder(),
  );
}

/// Creates a [Write] instruction.
Instruction getWriteInstruction({
  required Address programAddress,
  required Address authority,
  required Address journal,
  required int index,
  required BigInt value,
}) {
  final instructionData = WriteInstructionData(index: index, value: value);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: journal, role: AccountRole.writable),
    ],
    data: getWriteInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Write] instruction from raw instruction data.
WriteInstructionData parseWriteInstruction(Instruction instruction) {
  return getWriteInstructionDataDecoder().decode(instruction.data!);
}
