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
class RegisterAssetInstructionData {
  const RegisterAssetInstructionData({
    required this.bump,
  }) :
      discriminator = 13;

  final int discriminator;
  final int bump;
}

Encoder<RegisterAssetInstructionData> getRegisterAssetInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RegisterAssetInstructionData value) => <String, Object?>{
      'discriminator': 13,
      'bump': value.bump,
    },
  );
}

Decoder<RegisterAssetInstructionData> getRegisterAssetInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'registerAsset instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RegisterAssetInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(13),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RegisterAssetInstructionData(
      bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RegisterAssetInstructionData>(
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
      VariableSizeDecoder<RegisterAssetInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RegisterAssetInstructionData, RegisterAssetInstructionData> getRegisterAssetInstructionDataCodec() {
  return combineCodec(getRegisterAssetInstructionDataEncoder(), getRegisterAssetInstructionDataDecoder());
}

/// Creates a [RegisterAsset] instruction.
Instruction getRegisterAssetInstruction({
  required Address programAddress,
  required Address payer,
  required Address mint,
  required Address assetVault,
  required Address vaultTokenAccount,
  required Address tokenProgram,
  required Address systemProgram,
  required int bump,
}) {
  final instructionData = RegisterAssetInstructionData(
      bump: bump,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: payer, role: AccountRole.writableSigner),
    AccountMeta(address: mint, role: AccountRole.readonly),
    AccountMeta(address: assetVault, role: AccountRole.writable),
    AccountMeta(address: vaultTokenAccount, role: AccountRole.readonly),
    AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getRegisterAssetInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RegisterAsset] instruction from raw instruction data.
RegisterAssetInstructionData parseRegisterAssetInstruction(Instruction instruction) {
  return getRegisterAssetInstructionDataDecoder().decode(instruction.data!);
}
