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
class ForwardRotateWithPdaInstructionData {
  const ForwardRotateWithPdaInstructionData({
    required this.bump,
    required this.newAuthority,
  }) : discriminator = 2;

  final int discriminator;
  final int bump;
  final Address newAuthority;
}

Encoder<ForwardRotateWithPdaInstructionData>
getForwardRotateWithPdaInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('newAuthority', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ForwardRotateWithPdaInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'bump': value.bump,
      'newAuthority': value.newAuthority,
    },
  );
}

Decoder<ForwardRotateWithPdaInstructionData>
getForwardRotateWithPdaInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('newAuthority', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'forwardRotateWithPda instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ForwardRotateWithPdaInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ForwardRotateWithPdaInstructionData(
        bump: map['bump']! as int,
        newAuthority: map['newAuthority']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ForwardRotateWithPdaInstructionData>(
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
      VariableSizeDecoder<ForwardRotateWithPdaInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ForwardRotateWithPdaInstructionData, ForwardRotateWithPdaInstructionData>
getForwardRotateWithPdaInstructionDataCodec() {
  return combineCodec(
    getForwardRotateWithPdaInstructionDataEncoder(),
    getForwardRotateWithPdaInstructionDataDecoder(),
  );
}

/// Creates a [ForwardRotateWithPda] instruction.
Instruction getForwardRotateWithPdaInstruction({
  required Address programAddress,
  required Address oracle,
  required Address authority,
  required Address propAmmProgram,
  required int bump,
  required Address newAuthority,
}) {
  final instructionData = ForwardRotateWithPdaInstructionData(
    bump: bump,
    newAuthority: newAuthority,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: oracle, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonly),
      AccountMeta(address: propAmmProgram, role: AccountRole.readonly),
    ],
    data: getForwardRotateWithPdaInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [ForwardRotateWithPda] instruction from raw instruction data.
ForwardRotateWithPdaInstructionData parseForwardRotateWithPdaInstruction(
  Instruction instruction,
) {
  return getForwardRotateWithPdaInstructionDataDecoder().decode(
    instruction.data!,
  );
}
