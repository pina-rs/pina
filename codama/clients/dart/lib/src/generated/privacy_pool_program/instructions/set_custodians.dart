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
class SetCustodiansInstructionData {
  const SetCustodiansInstructionData({required this.custodians})
    : discriminator = 2,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Uint8List custodians;
}

Encoder<SetCustodiansInstructionData> getSetCustodiansInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    (
      'custodians',
      fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (SetCustodiansInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'migrationVersion': 0,
      'custodians': value.custodians,
    },
  );
}

Decoder<SetCustodiansInstructionData> getSetCustodiansInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('custodians', fixDecoderSize(getBytesDecoder(), 96)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'setCustodians instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (SetCustodiansInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      SetCustodiansInstructionData(custodians: map['custodians']! as Uint8List),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SetCustodiansInstructionData>(
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
      VariableSizeDecoder<SetCustodiansInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SetCustodiansInstructionData, SetCustodiansInstructionData>
getSetCustodiansInstructionDataCodec() {
  return combineCodec(
    getSetCustodiansInstructionDataEncoder(),
    getSetCustodiansInstructionDataDecoder(),
  );
}

/// Creates a [SetCustodians] instruction.
Instruction getSetCustodiansInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required Address custodianRegistry,
  required Uint8List custodians,
}) {
  final instructionData = SetCustodiansInstructionData(custodians: custodians);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: poolConfig, role: AccountRole.readonly),
      AccountMeta(address: custodianRegistry, role: AccountRole.writable),
    ],
    data: getSetCustodiansInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [SetCustodians] instruction from raw instruction data.
SetCustodiansInstructionData parseSetCustodiansInstruction(
  Instruction instruction,
) {
  return getSetCustodiansInstructionDataDecoder().decode(instruction.data!);
}
