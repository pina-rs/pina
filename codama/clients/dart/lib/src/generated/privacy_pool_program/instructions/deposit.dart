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
class DepositInstructionData {
  const DepositInstructionData({
    required this.bump,
    required this.commitment,
    required this.viewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
  }) :
      discriminator = 4,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Uint8List commitment;
  final Uint8List viewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;
}

Encoder<DepositInstructionData> getDepositInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('commitment', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('viewPubkey', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('envelopeLen', getU8Encoder()),
    ('envelope', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('shares', fixEncoderSize(getBytesEncoder(), 144, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (DepositInstructionData value) => <String, Object?>{
      'discriminator': 4,
      'migrationVersion': 0,
      'bump': value.bump,
      'commitment': value.commitment,
      'viewPubkey': value.viewPubkey,
      'envelopeLen': value.envelopeLen,
      'envelope': value.envelope,
      'shares': value.shares,
    },
  );
}

Decoder<DepositInstructionData> getDepositInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('viewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('envelopeLen', getU8Decoder()),
    ('envelope', fixDecoderSize(getBytesDecoder(), 128)),
    ('shares', fixDecoderSize(getBytesDecoder(), 144)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'deposit instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (DepositInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(4),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      DepositInstructionData(
      bump: map['bump']! as int,
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
      FixedSizeDecoder<DepositInstructionData>(
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
      VariableSizeDecoder<DepositInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DepositInstructionData, DepositInstructionData> getDepositInstructionDataCodec() {
  return combineCodec(getDepositInstructionDataEncoder(), getDepositInstructionDataDecoder());
}

/// Creates a [Deposit] instruction.
Instruction getDepositInstruction({
  required Address programAddress,
  required Address depositor,
  required Address poolConfig,
  required Address poolVault,
  required Address merkleTree,
  required Address noteCommitment,
  required Address systemProgram,
  required int bump,
  required Uint8List commitment,
  required Uint8List viewPubkey,
  required int envelopeLen,
  required Uint8List envelope,
  required Uint8List shares,
}) {
  final instructionData = DepositInstructionData(
      bump: bump,
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
    AccountMeta(address: poolVault, role: AccountRole.writable),
    AccountMeta(address: merkleTree, role: AccountRole.writable),
    AccountMeta(address: noteCommitment, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getDepositInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Deposit] instruction from raw instruction data.
DepositInstructionData parseDepositInstruction(Instruction instruction) {
  return getDepositInstructionDataDecoder().decode(instruction.data!);
}
