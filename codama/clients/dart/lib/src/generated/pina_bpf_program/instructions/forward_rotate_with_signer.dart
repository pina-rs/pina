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
class ForwardRotateWithSignerInstructionData {
  const ForwardRotateWithSignerInstructionData({required this.newAuthority})
    : discriminator = 1;

  final int discriminator;
  final Address newAuthority;
}

Encoder<ForwardRotateWithSignerInstructionData>
getForwardRotateWithSignerInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('newAuthority', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ForwardRotateWithSignerInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'newAuthority': value.newAuthority,
    },
  );
}

Decoder<ForwardRotateWithSignerInstructionData>
getForwardRotateWithSignerInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('newAuthority', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'forwardRotateWithSigner instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ForwardRotateWithSignerInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ForwardRotateWithSignerInstructionData(
        newAuthority: map['newAuthority']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ForwardRotateWithSignerInstructionData>(
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
      VariableSizeDecoder<ForwardRotateWithSignerInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<
  ForwardRotateWithSignerInstructionData,
  ForwardRotateWithSignerInstructionData
>
getForwardRotateWithSignerInstructionDataCodec() {
  return combineCodec(
    getForwardRotateWithSignerInstructionDataEncoder(),
    getForwardRotateWithSignerInstructionDataDecoder(),
  );
}

/// Creates a [ForwardRotateWithSigner] instruction.
Instruction getForwardRotateWithSignerInstruction({
  required Address programAddress,
  required Address oracle,
  required Address authority,
  required Address propAmmProgram,
  required Address newAuthority,
}) {
  final instructionData = ForwardRotateWithSignerInstructionData(
    newAuthority: newAuthority,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: oracle, role: AccountRole.writable),
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: propAmmProgram, role: AccountRole.readonly),
    ],
    data: getForwardRotateWithSignerInstructionDataEncoder().encode(
      instructionData,
    ),
  );
}

/// Parses a [ForwardRotateWithSigner] instruction from raw instruction data.
ForwardRotateWithSignerInstructionData parseForwardRotateWithSignerInstruction(
  Instruction instruction,
) {
  return getForwardRotateWithSignerInstructionDataDecoder().decode(
    instruction.data!,
  );
}
