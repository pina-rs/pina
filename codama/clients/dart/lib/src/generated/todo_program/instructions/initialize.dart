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
  const InitializeInstructionData({required this.bump, required this.digest})
    : discriminator = 0;

  final int discriminator;
  final int bump;
  final Uint8List digest;
}

Encoder<InitializeInstructionData> getInitializeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('digest', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializeInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'bump': value.bump,
      'digest': value.digest,
    },
  );
}

Decoder<InitializeInstructionData> getInitializeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('digest', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'initialize instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InitializeInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      InitializeInstructionData(
        bump: map['bump']! as int,
        digest: map['digest']! as Uint8List,
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
          return readExact(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<InitializeInstructionData>(
        read: readExact,
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
  required Address owner,
  required Address todo,
  required Address systemProgram,
  required int bump,
  required Uint8List digest,
}) {
  final instructionData = InitializeInstructionData(bump: bump, digest: digest);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: owner, role: AccountRole.readonlySigner),
      AccountMeta(address: todo, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getInitializeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Initialize] instruction from raw instruction data.
InitializeInstructionData parseInitializeInstruction(Instruction instruction) {
  return getInitializeInstructionDataDecoder().decode(instruction.data!);
}
