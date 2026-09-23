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
class TransferInstructionData {
  const TransferInstructionData({
    required this.bump,
    required this.nullifier,
    required this.root,
    required this.newCommitment,
    required this.newViewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
    required this.proofA,
    required this.proofB,
    required this.proofC,
  }) :
      discriminator = 6,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Uint8List nullifier;
  final Uint8List root;
  final Uint8List newCommitment;
  final Uint8List newViewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;
  final Uint8List proofA;
  final Uint8List proofB;
  final Uint8List proofC;
}

Encoder<TransferInstructionData> getTransferInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('nullifier', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('root', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('newCommitment', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('newViewPubkey', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('envelopeLen', getU8Encoder()),
    ('envelope', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('shares', fixEncoderSize(getBytesEncoder(), 144, allowTruncation: false)),
    ('proofA', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('proofB', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('proofC', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (TransferInstructionData value) => <String, Object?>{
      'discriminator': 6,
      'migrationVersion': 0,
      'bump': value.bump,
      'nullifier': value.nullifier,
      'root': value.root,
      'newCommitment': value.newCommitment,
      'newViewPubkey': value.newViewPubkey,
      'envelopeLen': value.envelopeLen,
      'envelope': value.envelope,
      'shares': value.shares,
      'proofA': value.proofA,
      'proofB': value.proofB,
      'proofC': value.proofC,
    },
  );
}

Decoder<TransferInstructionData> getTransferInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('nullifier', fixDecoderSize(getBytesDecoder(), 32)),
    ('root', fixDecoderSize(getBytesDecoder(), 32)),
    ('newCommitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('newViewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('envelopeLen', getU8Decoder()),
    ('envelope', fixDecoderSize(getBytesDecoder(), 128)),
    ('shares', fixDecoderSize(getBytesDecoder(), 144)),
    ('proofA', fixDecoderSize(getBytesDecoder(), 64)),
    ('proofB', fixDecoderSize(getBytesDecoder(), 128)),
    ('proofC', fixDecoderSize(getBytesDecoder(), 64)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'transfer instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (TransferInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(6),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      TransferInstructionData(
      bump: map['bump']! as int,
      nullifier: map['nullifier']! as Uint8List,
      root: map['root']! as Uint8List,
      newCommitment: map['newCommitment']! as Uint8List,
      newViewPubkey: map['newViewPubkey']! as Uint8List,
      envelopeLen: map['envelopeLen']! as int,
      envelope: map['envelope']! as Uint8List,
      shares: map['shares']! as Uint8List,
      proofA: map['proofA']! as Uint8List,
      proofB: map['proofB']! as Uint8List,
      proofC: map['proofC']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<TransferInstructionData>(
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
      VariableSizeDecoder<TransferInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TransferInstructionData, TransferInstructionData> getTransferInstructionDataCodec() {
  return combineCodec(getTransferInstructionDataEncoder(), getTransferInstructionDataDecoder());
}

/// Creates a [Transfer] instruction.
Instruction getTransferInstruction({
  required Address programAddress,
  required Address poolConfig,
  required Address payer,
  required Address merkleTree,
  required Address nullifierSet,
  required Address verifyingKeyAccount,
  required Address noteCommitment,
  required Address systemProgram,
  required int bump,
  required Uint8List nullifier,
  required Uint8List root,
  required Uint8List newCommitment,
  required Uint8List newViewPubkey,
  required int envelopeLen,
  required Uint8List envelope,
  required Uint8List shares,
  required Uint8List proofA,
  required Uint8List proofB,
  required Uint8List proofC,
}) {
  final instructionData = TransferInstructionData(
      bump: bump,
      nullifier: nullifier,
      root: root,
      newCommitment: newCommitment,
      newViewPubkey: newViewPubkey,
      envelopeLen: envelopeLen,
      envelope: envelope,
      shares: shares,
      proofA: proofA,
      proofB: proofB,
      proofC: proofC,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: payer, role: AccountRole.writableSigner),
    AccountMeta(address: merkleTree, role: AccountRole.writable),
    AccountMeta(address: nullifierSet, role: AccountRole.writable),
    AccountMeta(address: verifyingKeyAccount, role: AccountRole.readonly),
    AccountMeta(address: noteCommitment, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getTransferInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Transfer] instruction from raw instruction data.
TransferInstructionData parseTransferInstruction(Instruction instruction) {
  return getTransferInstructionDataDecoder().decode(instruction.data!);
}
