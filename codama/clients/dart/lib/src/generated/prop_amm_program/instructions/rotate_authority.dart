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
class RotateAuthorityInstructionData {
  const RotateAuthorityInstructionData({required this.newAuthority})
    : discriminator = 2;

  final int discriminator;
  final Address newAuthority;
}

Encoder<RotateAuthorityInstructionData>
getRotateAuthorityInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('newAuthority', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RotateAuthorityInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'newAuthority': value.newAuthority,
    },
  );
}

Decoder<RotateAuthorityInstructionData>
getRotateAuthorityInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('newAuthority', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'rotateAuthority instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RotateAuthorityInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RotateAuthorityInstructionData(
        newAuthority: map['newAuthority']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RotateAuthorityInstructionData>(
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
      VariableSizeDecoder<RotateAuthorityInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RotateAuthorityInstructionData, RotateAuthorityInstructionData>
getRotateAuthorityInstructionDataCodec() {
  return combineCodec(
    getRotateAuthorityInstructionDataEncoder(),
    getRotateAuthorityInstructionDataDecoder(),
  );
}

/// Creates a [RotateAuthority] instruction.
Instruction getRotateAuthorityInstruction({
  required Address programAddress,
  required Address oracle,
  required Address authority,
  required Address newAuthority,
}) {
  final instructionData = RotateAuthorityInstructionData(
    newAuthority: newAuthority,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: oracle, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
    ],
    data: getRotateAuthorityInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RotateAuthority] instruction from raw instruction data.
RotateAuthorityInstructionData parseRotateAuthorityInstruction(
  Instruction instruction,
) {
  return getRotateAuthorityInstructionDataDecoder().decode(instruction.data!);
}
