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
class CpiTransferInstructionData {
  const CpiTransferInstructionData({required this.amount}) : discriminator = 0;

  final int discriminator;
  final BigInt amount;
}

Encoder<CpiTransferInstructionData> getCpiTransferInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CpiTransferInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'amount': value.amount,
    },
  );
}

Decoder<CpiTransferInstructionData> getCpiTransferInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'cpiTransfer instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CpiTransferInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      CpiTransferInstructionData(amount: map['amount']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CpiTransferInstructionData>(
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
      VariableSizeDecoder<CpiTransferInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CpiTransferInstructionData, CpiTransferInstructionData>
getCpiTransferInstructionDataCodec() {
  return combineCodec(
    getCpiTransferInstructionDataEncoder(),
    getCpiTransferInstructionDataDecoder(),
  );
}

/// Creates a [CpiTransfer] instruction.
Instruction getCpiTransferInstruction({
  required Address programAddress,
  required Address sender,
  required Address recipient,
  required Address systemProgram,
  required BigInt amount,
}) {
  final instructionData = CpiTransferInstructionData(amount: amount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: sender, role: AccountRole.writableSigner),
      AccountMeta(address: recipient, role: AccountRole.writable),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getCpiTransferInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [CpiTransfer] instruction from raw instruction data.
CpiTransferInstructionData parseCpiTransferInstruction(
  Instruction instruction,
) {
  return getCpiTransferInstructionDataDecoder().decode(instruction.data!);
}
