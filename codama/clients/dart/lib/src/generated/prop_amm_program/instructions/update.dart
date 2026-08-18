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
  const UpdateInstructionData({required this.newPrice}) : discriminator = 1;

  final int discriminator;
  final BigInt newPrice;
}

Encoder<UpdateInstructionData> getUpdateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('newPrice', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'newPrice': value.newPrice,
    },
  );
}

Decoder<UpdateInstructionData> getUpdateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('newPrice', getU64Decoder()),
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
      UpdateInstructionData(newPrice: map['newPrice']! as BigInt),
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
  required Address oracle,
  required Address authority,
  required BigInt newPrice,
}) {
  final instructionData = UpdateInstructionData(newPrice: newPrice);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: oracle, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
    ],
    data: getUpdateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Update] instruction from raw instruction data.
UpdateInstructionData parseUpdateInstruction(Instruction instruction) {
  return getUpdateInstructionDataDecoder().decode(instruction.data!);
}
