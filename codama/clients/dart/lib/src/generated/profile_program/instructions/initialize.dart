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
class InitializeInstructionData {
  const InitializeInstructionData({
    required this.bump,
    required this.name,
    required this.bio,
  }) : discriminator = 0;

  final int discriminator;
  final int bump;
  final Uint8List name;
  final Uint8List bio;
}

Encoder<InitializeInstructionData> getInitializeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('name', fixEncoderSize(getBytesEncoder(), 33, allowTruncation: false)),
    ('bio', fixEncoderSize(getBytesEncoder(), 129, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializeInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'bump': value.bump,
      'name': value.name,
      'bio': value.bio,
    },
  );
}

Decoder<InitializeInstructionData> getInitializeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('name', fixDecoderSize(getBytesDecoder(), 33)),
    ('bio', fixDecoderSize(getBytesDecoder(), 129)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'initialize instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InitializeInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      InitializeInstructionData(
        bump: map['bump']! as int,
        name: map['name']! as Uint8List,
        bio: map['bio']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitializeInstructionData>(
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
      VariableSizeDecoder<InitializeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitializeInstructionData, InitializeInstructionData>
getInitializeInstructionDataCodec() {
  return combineCodec(
    getInitializeInstructionDataEncoder(),
    getInitializeInstructionDataDecoder(),
  );
}

/// Creates a [Initialize] instruction.
Instruction getInitializeInstruction({
  required Address programAddress,
  required Address authority,
  required Address profile,
  required Address systemProgram,
  required int bump,
  required Uint8List name,
  required Uint8List bio,
}) {
  final instructionData = InitializeInstructionData(
    bump: bump,
    name: name,
    bio: bio,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.writableSigner),
      AccountMeta(address: profile, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getInitializeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Initialize] instruction from raw instruction data.
InitializeInstructionData parseInitializeInstruction(Instruction instruction) {
  return getInitializeInstructionDataDecoder().decode(instruction.data!);
}
