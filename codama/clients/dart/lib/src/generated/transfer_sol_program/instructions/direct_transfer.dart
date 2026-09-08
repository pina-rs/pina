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
class DirectTransferInstructionData {
  const DirectTransferInstructionData({required this.amount})
    : discriminator = 1;

  final int discriminator;
  final BigInt amount;
}

Encoder<DirectTransferInstructionData>
getDirectTransferInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (DirectTransferInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'amount': value.amount,
    },
  );
}

Decoder<DirectTransferInstructionData>
getDirectTransferInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'directTransfer instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (DirectTransferInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      DirectTransferInstructionData(amount: map['amount']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<DirectTransferInstructionData>(
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
      VariableSizeDecoder<DirectTransferInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DirectTransferInstructionData, DirectTransferInstructionData>
getDirectTransferInstructionDataCodec() {
  return combineCodec(
    getDirectTransferInstructionDataEncoder(),
    getDirectTransferInstructionDataDecoder(),
  );
}

/// Creates a [DirectTransfer] instruction.
Instruction getDirectTransferInstruction({
  required Address programAddress,
  required Address sender,
  required Address recipient,
  required BigInt amount,
}) {
  final instructionData = DirectTransferInstructionData(amount: amount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: sender, role: AccountRole.writableSigner),
      AccountMeta(address: recipient, role: AccountRole.writable),
    ],
    data: getDirectTransferInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [DirectTransfer] instruction from raw instruction data.
DirectTransferInstructionData parseDirectTransferInstruction(
  Instruction instruction,
) {
  return getDirectTransferInstructionDataDecoder().decode(instruction.data!);
}
