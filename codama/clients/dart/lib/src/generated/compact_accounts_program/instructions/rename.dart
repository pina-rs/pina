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
class RenameInstructionData {
  const RenameInstructionData({
    required this.titleLen,
    required this.title,
  }) :
      discriminator = 3;

  final int discriminator;
  final int titleLen;
  final Uint8List title;
}

Encoder<RenameInstructionData> getRenameInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('titleLen', getU8Encoder()),
    ('title', fixEncoderSize(getBytesEncoder(), 24, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (RenameInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'titleLen': value.titleLen,
      'title': value.title,
    },
  );
}

Decoder<RenameInstructionData> getRenameInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('titleLen', getU8Decoder()),
    ('title', fixDecoderSize(getBytesDecoder(), 24)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'rename instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RenameInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(3),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RenameInstructionData(
      titleLen: map['titleLen']! as int,
      title: map['title']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RenameInstructionData>(
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
      VariableSizeDecoder<RenameInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RenameInstructionData, RenameInstructionData> getRenameInstructionDataCodec() {
  return combineCodec(getRenameInstructionDataEncoder(), getRenameInstructionDataDecoder());
}

/// Creates a [Rename] instruction.
Instruction getRenameInstruction({
  required Address programAddress,
  required Address authority,
  required Address journal,
  required Address systemProgram,
  required int titleLen,
  required Uint8List title,
}) {
  final instructionData = RenameInstructionData(
      titleLen: titleLen,
      title: title,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.writableSigner),
    AccountMeta(address: journal, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getRenameInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Rename] instruction from raw instruction data.
RenameInstructionData parseRenameInstruction(Instruction instruction) {
  return getRenameInstructionDataDecoder().decode(instruction.data!);
}
