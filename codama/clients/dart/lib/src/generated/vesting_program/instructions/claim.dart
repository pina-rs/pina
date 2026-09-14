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
class ClaimInstructionData {
  const ClaimInstructionData({required this.amount}) : discriminator = 1;

  final int discriminator;
  final BigInt amount;
}

Encoder<ClaimInstructionData> getClaimInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ClaimInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'amount': value.amount,
    },
  );
}

Decoder<ClaimInstructionData> getClaimInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'claim instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ClaimInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ClaimInstructionData(amount: map['amount']! as BigInt), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ClaimInstructionData>(
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
      VariableSizeDecoder<ClaimInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ClaimInstructionData, ClaimInstructionData>
getClaimInstructionDataCodec() {
  return combineCodec(
    getClaimInstructionDataEncoder(),
    getClaimInstructionDataDecoder(),
  );
}

/// Creates a [Claim] instruction.
Instruction getClaimInstruction({
  required Address programAddress,
  required Address beneficiary,
  required Address mint,
  required Address vestingState,
  required Address beneficiaryAta,
  required Address vault,
  required Address associatedTokenProgram,
  required Address systemProgram,
  required Address tokenProgram,
  required BigInt amount,
}) {
  final instructionData = ClaimInstructionData(amount: amount);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: beneficiary, role: AccountRole.writableSigner),
      AccountMeta(address: mint, role: AccountRole.readonly),
      AccountMeta(address: vestingState, role: AccountRole.writable),
      AccountMeta(address: beneficiaryAta, role: AccountRole.writable),
      AccountMeta(address: vault, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getClaimInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Claim] instruction from raw instruction data.
ClaimInstructionData parseClaimInstruction(Instruction instruction) {
  return getClaimInstructionDataDecoder().decode(instruction.data!);
}
