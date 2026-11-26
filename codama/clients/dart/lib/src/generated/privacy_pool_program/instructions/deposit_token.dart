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
class DepositTokenInstructionData {
  const DepositTokenInstructionData({
    required this.amount,
    required this.commitment,
    required this.viewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
  }) : discriminator = 14;

  final int discriminator;
  final BigInt amount;
  final Uint8List commitment;
  final Uint8List viewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;
}

Encoder<DepositTokenInstructionData> getDepositTokenInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
    (
      'commitment',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    (
      'viewPubkey',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('envelopeLen', getU8Encoder()),
    ('envelope', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
    ('shares', fixEncoderSize(getBytesEncoder(), 144, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (DepositTokenInstructionData value) => <String, Object?>{
      'discriminator': 14,
      'amount': value.amount,
      'commitment': value.commitment,
      'viewPubkey': value.viewPubkey,
      'envelopeLen': value.envelopeLen,
      'envelope': value.envelope,
      'shares': value.shares,
    },
  );
}

Decoder<DepositTokenInstructionData> getDepositTokenInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
    ('commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('viewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('envelopeLen', getU8Decoder()),
    ('envelope', fixDecoderSize(getBytesDecoder(), 96)),
    ('shares', fixDecoderSize(getBytesDecoder(), 144)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'depositToken instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (DepositTokenInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(14)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      DepositTokenInstructionData(
        amount: map['amount']! as BigInt,
        commitment: map['commitment']! as Uint8List,
        viewPubkey: map['viewPubkey']! as Uint8List,
        envelopeLen: map['envelopeLen']! as int,
        envelope: map['envelope']! as Uint8List,
        shares: map['shares']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<DepositTokenInstructionData>(
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
      VariableSizeDecoder<DepositTokenInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DepositTokenInstructionData, DepositTokenInstructionData>
getDepositTokenInstructionDataCodec() {
  return combineCodec(
    getDepositTokenInstructionDataEncoder(),
    getDepositTokenInstructionDataDecoder(),
  );
}

/// Creates a [DepositToken] instruction.
Instruction getDepositTokenInstruction({
  required Address programAddress,
  required Address depositor,
  required Address poolConfig,
  required Address merkleTree,
  required Address mint,
  required Address assetVault,
  required Address depositorTokenAccount,
  required Address vaultTokenAccount,
  required Address tokenProgram,
  required BigInt amount,
  required Uint8List commitment,
  required Uint8List viewPubkey,
  required int envelopeLen,
  required Uint8List envelope,
  required Uint8List shares,
}) {
  final instructionData = DepositTokenInstructionData(
    amount: amount,
    commitment: commitment,
    viewPubkey: viewPubkey,
    envelopeLen: envelopeLen,
    envelope: envelope,
    shares: shares,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: depositor, role: AccountRole.writableSigner),
      AccountMeta(address: poolConfig, role: AccountRole.writable),
      AccountMeta(address: merkleTree, role: AccountRole.writable),
      AccountMeta(address: mint, role: AccountRole.readonly),
      AccountMeta(address: assetVault, role: AccountRole.readonly),
      AccountMeta(address: depositorTokenAccount, role: AccountRole.writable),
      AccountMeta(address: vaultTokenAccount, role: AccountRole.writable),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getDepositTokenInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [DepositToken] instruction from raw instruction data.
DepositTokenInstructionData parseDepositTokenInstruction(
  Instruction instruction,
) {
  return getDepositTokenInstructionDataDecoder().decode(instruction.data!);
}
