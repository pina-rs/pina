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
class ConfigExecuteInstructionData {
  const ConfigExecuteInstructionData()
    : discriminator = 11,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
}

Encoder<ConfigExecuteInstructionData> getConfigExecuteInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ConfigExecuteInstructionData value) => <String, Object?>{
      'discriminator': 11,
      'migrationVersion': 0,
    },
  );
}

Decoder<ConfigExecuteInstructionData> getConfigExecuteInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'configExecute instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ConfigExecuteInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(11)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ConfigExecuteInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ConfigExecuteInstructionData>(
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
      VariableSizeDecoder<ConfigExecuteInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ConfigExecuteInstructionData, ConfigExecuteInstructionData>
getConfigExecuteInstructionDataCodec() {
  return combineCodec(
    getConfigExecuteInstructionDataEncoder(),
    getConfigExecuteInstructionDataDecoder(),
  );
}

/// Creates a [ConfigExecute] instruction.
Instruction getConfigExecuteInstruction({
  required Address programAddress,
  required Address multisig,
  required Address proposal,
  required Address member,
  required Address rentPayer,
  required Address systemProgram,
  required Address clock,
  required Address rentCollector,
  required Address spendingLimitAccounts,
}) {
  final instructionData = ConfigExecuteInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: multisig, role: AccountRole.writable),
      AccountMeta(address: proposal, role: AccountRole.writable),
      AccountMeta(address: member, role: AccountRole.readonlySigner),
      AccountMeta(address: rentPayer, role: AccountRole.writableSigner),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      AccountMeta(address: clock, role: AccountRole.readonly),
      AccountMeta(address: rentCollector, role: AccountRole.writable),
      AccountMeta(address: spendingLimitAccounts, role: AccountRole.writable),
    ],
    data: getConfigExecuteInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ConfigExecute] instruction from raw instruction data.
ConfigExecuteInstructionData parseConfigExecuteInstruction(
  Instruction instruction,
) {
  return getConfigExecuteInstructionDataDecoder().decode(instruction.data!);
}
