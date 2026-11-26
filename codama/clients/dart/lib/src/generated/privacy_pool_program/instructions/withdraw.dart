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
class WithdrawInstructionData {
  const WithdrawInstructionData({
    required this.nullifier0Bump,
    required this.nullifier1Bump,
    required this.nullifier0,
    required this.nullifier1,
    required this.root,
    required this.publicOut,
    required this.relayerFee,
    required this.changeCommitment,
    required this.changeViewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
    required this.proofA,
    required this.proofB,
    required this.proofC,
  }) : discriminator = 5;

  final int discriminator;
  final int nullifier0Bump;
  final int nullifier1Bump;
  final Uint8List nullifier0;
  final Uint8List nullifier1;
  final Uint8List root;
  final BigInt publicOut;
  final BigInt relayerFee;
  final Uint8List changeCommitment;
  final Uint8List changeViewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;
  final Uint8List proofA;
  final Uint8List proofB;
  final Uint8List proofC;
}

Encoder<WithdrawInstructionData> getWithdrawInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('nullifier0Bump', getU8Encoder()),
    ('nullifier1Bump', getU8Encoder()),
    (
      'nullifier0',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    (
      'nullifier1',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('root', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('publicOut', getU64Encoder()),
    ('relayerFee', getU64Encoder()),
    (
      'changeCommitment',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    (
      'changeViewPubkey',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('envelopeLen', getU8Encoder()),
    ('envelope', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
    ('shares', fixEncoderSize(getBytesEncoder(), 144, allowTruncation: false)),
    ('proofA', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('proofB', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('proofC', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (WithdrawInstructionData value) => <String, Object?>{
      'discriminator': 5,
      'nullifier0Bump': value.nullifier0Bump,
      'nullifier1Bump': value.nullifier1Bump,
      'nullifier0': value.nullifier0,
      'nullifier1': value.nullifier1,
      'root': value.root,
      'publicOut': value.publicOut,
      'relayerFee': value.relayerFee,
      'changeCommitment': value.changeCommitment,
      'changeViewPubkey': value.changeViewPubkey,
      'envelopeLen': value.envelopeLen,
      'envelope': value.envelope,
      'shares': value.shares,
      'proofA': value.proofA,
      'proofB': value.proofB,
      'proofC': value.proofC,
    },
  );
}

Decoder<WithdrawInstructionData> getWithdrawInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('nullifier0Bump', getU8Decoder()),
    ('nullifier1Bump', getU8Decoder()),
    ('nullifier0', fixDecoderSize(getBytesDecoder(), 32)),
    ('nullifier1', fixDecoderSize(getBytesDecoder(), 32)),
    ('root', fixDecoderSize(getBytesDecoder(), 32)),
    ('publicOut', getU64Decoder()),
    ('relayerFee', getU64Decoder()),
    ('changeCommitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('changeViewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('envelopeLen', getU8Decoder()),
    ('envelope', fixDecoderSize(getBytesDecoder(), 96)),
    ('shares', fixDecoderSize(getBytesDecoder(), 144)),
    ('proofA', fixDecoderSize(getBytesDecoder(), 32)),
    ('proofB', fixDecoderSize(getBytesDecoder(), 128)),
    ('proofC', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'withdraw instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (WithdrawInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(5)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      WithdrawInstructionData(
        nullifier0Bump: map['nullifier0Bump']! as int,
        nullifier1Bump: map['nullifier1Bump']! as int,
        nullifier0: map['nullifier0']! as Uint8List,
        nullifier1: map['nullifier1']! as Uint8List,
        root: map['root']! as Uint8List,
        publicOut: map['publicOut']! as BigInt,
        relayerFee: map['relayerFee']! as BigInt,
        changeCommitment: map['changeCommitment']! as Uint8List,
        changeViewPubkey: map['changeViewPubkey']! as Uint8List,
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
      FixedSizeDecoder<WithdrawInstructionData>(
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
      VariableSizeDecoder<WithdrawInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<WithdrawInstructionData, WithdrawInstructionData>
getWithdrawInstructionDataCodec() {
  return combineCodec(
    getWithdrawInstructionDataEncoder(),
    getWithdrawInstructionDataDecoder(),
  );
}

/// Creates a [Withdraw] instruction.
Instruction getWithdrawInstruction({
  required Address programAddress,
  required Address payer,
  required Address poolConfig,
  required Address poolVault,
  required Address merkleTree,
  required Address nullifierRecord0,
  required Address nullifierRecord1,
  required Address verifyingKeyAccount,
  required Address recipient,
  required Address policyCosigner,
  required Address systemProgram,
  required int nullifier0Bump,
  required int nullifier1Bump,
  required Uint8List nullifier0,
  required Uint8List nullifier1,
  required Uint8List root,
  required BigInt publicOut,
  required BigInt relayerFee,
  required Uint8List changeCommitment,
  required Uint8List changeViewPubkey,
  required int envelopeLen,
  required Uint8List envelope,
  required Uint8List shares,
  required Uint8List proofA,
  required Uint8List proofB,
  required Uint8List proofC,
}) {
  final instructionData = WithdrawInstructionData(
    nullifier0Bump: nullifier0Bump,
    nullifier1Bump: nullifier1Bump,
    nullifier0: nullifier0,
    nullifier1: nullifier1,
    root: root,
    publicOut: publicOut,
    relayerFee: relayerFee,
    changeCommitment: changeCommitment,
    changeViewPubkey: changeViewPubkey,
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
      AccountMeta(address: payer, role: AccountRole.writableSigner),
      AccountMeta(address: poolConfig, role: AccountRole.readonly),
      AccountMeta(address: poolVault, role: AccountRole.writable),
      AccountMeta(address: merkleTree, role: AccountRole.writable),
      AccountMeta(address: nullifierRecord0, role: AccountRole.writable),
      AccountMeta(address: nullifierRecord1, role: AccountRole.writable),
      AccountMeta(address: verifyingKeyAccount, role: AccountRole.readonly),
      AccountMeta(address: recipient, role: AccountRole.writable),
      AccountMeta(address: policyCosigner, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getWithdrawInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Withdraw] instruction from raw instruction data.
WithdrawInstructionData parseWithdrawInstruction(Instruction instruction) {
  return getWithdrawInstructionDataDecoder().decode(instruction.data!);
}
