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
class InitializeInstructionData {
  const InitializeInstructionData({
    required this.totalAmount,
    required this.startTs,
    required this.cliffTs,
    required this.endTs,
    required this.bump,
  }) : discriminator = 0;

  final int discriminator;
  final BigInt totalAmount;
  final BigInt startTs;
  final BigInt cliffTs;
  final BigInt endTs;
  final int bump;
}

Encoder<InitializeInstructionData> getInitializeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('totalAmount', getU64Encoder()),
    ('startTs', getU64Encoder()),
    ('cliffTs', getU64Encoder()),
    ('endTs', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializeInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'totalAmount': value.totalAmount,
      'startTs': value.startTs,
      'cliffTs': value.cliffTs,
      'endTs': value.endTs,
      'bump': value.bump,
    },
  );
}

Decoder<InitializeInstructionData> getInitializeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('totalAmount', getU64Decoder()),
    ('startTs', getU64Decoder()),
    ('cliffTs', getU64Decoder()),
    ('endTs', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'initialize instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InitializeInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      InitializeInstructionData(
        totalAmount: map['totalAmount']! as BigInt,
        startTs: map['startTs']! as BigInt,
        cliffTs: map['cliffTs']! as BigInt,
        endTs: map['endTs']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitializeInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readExact(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<InitializeInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitializeInstructionData, InitializeInstructionData>
getInitializeInstructionDataCodec() {
  return combineCodec(
    getInitializeInstructionDataEncoder(),
    getInitializeInstructionDataDecoder(),
  );
}

/// Creates a [Initialize] instruction.
Instruction getInitializeInstruction({
  required Address programAddress,
  required Address admin,
  required Address beneficiary,
  required Address mint,
  required Address vestingState,
  required Address vault,
  required Address associatedTokenProgram,
  required Address systemProgram,
  required Address tokenProgram,
  required BigInt totalAmount,
  required BigInt startTs,
  required BigInt cliffTs,
  required BigInt endTs,
  required int bump,
}) {
  final instructionData = InitializeInstructionData(
    totalAmount: totalAmount,
    startTs: startTs,
    cliffTs: cliffTs,
    endTs: endTs,
    bump: bump,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.writableSigner),
      AccountMeta(address: beneficiary, role: AccountRole.readonly),
      AccountMeta(address: mint, role: AccountRole.readonly),
      AccountMeta(address: vestingState, role: AccountRole.writable),
      AccountMeta(address: vault, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getInitializeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Initialize] instruction from raw instruction data.
InitializeInstructionData parseInitializeInstruction(Instruction instruction) {
  return getInitializeInstructionDataDecoder().decode(instruction.data!);
}
