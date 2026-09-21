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
class RelayInstructionData {
  const RelayInstructionData({
    required this.value,
  }) :
      discriminator = 1;

  final int discriminator;
  final BigInt value;
}

Encoder<RelayInstructionData> getRelayInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('value', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RelayInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'value': value.value,
    },
  );
}

Decoder<RelayInstructionData> getRelayInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('value', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'relay instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RelayInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(1),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RelayInstructionData(
      value: map['value']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RelayInstructionData>(
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
      VariableSizeDecoder<RelayInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RelayInstructionData, RelayInstructionData> getRelayInstructionDataCodec() {
  return combineCodec(getRelayInstructionDataEncoder(), getRelayInstructionDataDecoder());
}

/// Creates a [Relay] instruction.
Instruction getRelayInstruction({
  required Address programAddress,
  required Address authority,
  required Address referrer,
  required Address state,
  required Address migrationPayer,
  required Address systemProgram,
  required Address migrationProgram,
  required BigInt value,
}) {
  final instructionData = RelayInstructionData(
      value: value,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.readonlySigner),
    AccountMeta(address: referrer, role: AccountRole.readonly),
    AccountMeta(address: state, role: AccountRole.writable),
    AccountMeta(address: migrationPayer, role: AccountRole.writableSigner),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    AccountMeta(address: migrationProgram, role: AccountRole.readonly),
    ],
    data: getRelayInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Relay] instruction from raw instruction data.
RelayInstructionData parseRelayInstruction(Instruction instruction) {
  return getRelayInstructionDataDecoder().decode(instruction.data!);
}
