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
class UpdateDigestInstructionData {
  const UpdateDigestInstructionData({required this.digest}) : discriminator = 2;

  final int discriminator;
  final Uint8List digest;
}

Encoder<UpdateDigestInstructionData> getUpdateDigestInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('digest', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateDigestInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'digest': value.digest,
    },
  );
}

Decoder<UpdateDigestInstructionData> getUpdateDigestInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('digest', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'updateDigest instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (UpdateDigestInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      UpdateDigestInstructionData(digest: map['digest']! as Uint8List),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<UpdateDigestInstructionData>(
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
      VariableSizeDecoder<UpdateDigestInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<UpdateDigestInstructionData, UpdateDigestInstructionData>
getUpdateDigestInstructionDataCodec() {
  return combineCodec(
    getUpdateDigestInstructionDataEncoder(),
    getUpdateDigestInstructionDataDecoder(),
  );
}

/// Creates a [UpdateDigest] instruction.
Instruction getUpdateDigestInstruction({
  required Address programAddress,
  required Address owner,
  required Address todo,
  required Uint8List digest,
}) {
  final instructionData = UpdateDigestInstructionData(digest: digest);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: owner, role: AccountRole.readonlySigner),
      AccountMeta(address: todo, role: AccountRole.writable),
    ],
    data: getUpdateDigestInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [UpdateDigest] instruction from raw instruction data.
UpdateDigestInstructionData parseUpdateDigestInstruction(
  Instruction instruction,
) {
  return getUpdateDigestInstructionDataDecoder().decode(instruction.data!);
}
