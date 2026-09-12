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
class UpdateInstructionData {
  const UpdateInstructionData({required this.value, required this.memo})
    : discriminator = 0,
      migrationVersion = 2;

  final int discriminator;
  final int migrationVersion;
  final BigInt value;
  final int memo;
}

Encoder<UpdateInstructionData> getUpdateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('value', getU64Encoder()),
    ('memo', getU16Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'migrationVersion': 2,
      'value': value.value,
      'memo': value.memo,
    },
  );
}

Decoder<UpdateInstructionData> getUpdateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('value', getU64Decoder()),
    ('memo', getU16Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'update instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (UpdateInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      UpdateInstructionData(
        value: map['value']! as BigInt,
        memo: map['memo']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<UpdateInstructionData>(
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
      VariableSizeDecoder<UpdateInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<UpdateInstructionData, UpdateInstructionData>
getUpdateInstructionDataCodec() {
  return combineCodec(
    getUpdateInstructionDataEncoder(),
    getUpdateInstructionDataDecoder(),
  );
}

/// Creates a [Update] instruction.
Instruction getUpdateInstruction({
  required Address programAddress,
  required Address authority,
  Address? referrer,
  Address? state,
  Address? migrationPayer,
  Address? systemProgram,
  Address? manualState,
  Address? compactState,
  required BigInt value,
  required int memo,
}) {
  final instructionData = UpdateInstructionData(value: value, memo: memo);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      if (referrer != null)
        AccountMeta(address: referrer, role: AccountRole.readonly)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (state != null)
        AccountMeta(address: state, role: AccountRole.writable)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (migrationPayer != null)
        AccountMeta(address: migrationPayer, role: AccountRole.writableSigner)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (systemProgram != null)
        AccountMeta(address: systemProgram, role: AccountRole.readonly)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (manualState != null)
        AccountMeta(address: manualState, role: AccountRole.writable)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      if (compactState != null)
        AccountMeta(address: compactState, role: AccountRole.writable)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
    ],
    data: getUpdateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Update] instruction from raw instruction data.
UpdateInstructionData parseUpdateInstruction(Instruction instruction) {
  return getUpdateInstructionDataDecoder().decode(instruction.data!);
}
