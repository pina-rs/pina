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
    required this.nullifier0Bump,
    required this.nullifier1Bump,
    required this.nullifier0,
    required this.nullifier1,
    required this.root,
    required this.relayerFee,
    required this.out0Commitment,
    required this.out0ViewPubkey,
    required this.out0EnvelopeLen,
    required this.out0Envelope,
    required this.out1Commitment,
    required this.out1ViewPubkey,
    required this.out1EnvelopeLen,
    required this.out1Envelope,
    required this.proofA,
    required this.proofB,
    required this.proofC,
  }) : discriminator = 6;

  final int discriminator;
  final int nullifier0Bump;
  final int nullifier1Bump;
  final Uint8List nullifier0;
  final Uint8List nullifier1;
  final Uint8List root;
  final BigInt relayerFee;
  final Uint8List out0Commitment;
  final Uint8List out0ViewPubkey;
  final int out0EnvelopeLen;
  final Uint8List out0Envelope;
  final Uint8List out1Commitment;
  final Uint8List out1ViewPubkey;
  final int out1EnvelopeLen;
  final Uint8List out1Envelope;
  final Uint8List proofA;
  final Uint8List proofB;
  final Uint8List proofC;
}

Encoder<TransferInstructionData> getTransferInstructionDataEncoder() {
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
    ('relayerFee', getU64Encoder()),
    (
      'out0Commitment',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    (
      'out0ViewPubkey',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('out0EnvelopeLen', getU8Encoder()),
    (
      'out0Envelope',
      fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false),
    ),
    (
      'out1Commitment',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    (
      'out1ViewPubkey',
      fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false),
    ),
    ('out1EnvelopeLen', getU8Encoder()),
    (
      'out1Envelope',
      fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false),
    ),
    ('proofA', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('proofB', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('proofC', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (TransferInstructionData value) => <String, Object?>{
      'discriminator': 6,
      'nullifier0Bump': value.nullifier0Bump,
      'nullifier1Bump': value.nullifier1Bump,
      'nullifier0': value.nullifier0,
      'nullifier1': value.nullifier1,
      'root': value.root,
      'relayerFee': value.relayerFee,
      'out0Commitment': value.out0Commitment,
      'out0ViewPubkey': value.out0ViewPubkey,
      'out0EnvelopeLen': value.out0EnvelopeLen,
      'out0Envelope': value.out0Envelope,
      'out1Commitment': value.out1Commitment,
      'out1ViewPubkey': value.out1ViewPubkey,
      'out1EnvelopeLen': value.out1EnvelopeLen,
      'out1Envelope': value.out1Envelope,
      'proofA': value.proofA,
      'proofB': value.proofB,
      'proofC': value.proofC,
    },
  );
}

Decoder<TransferInstructionData> getTransferInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('nullifier0Bump', getU8Decoder()),
    ('nullifier1Bump', getU8Decoder()),
    ('nullifier0', fixDecoderSize(getBytesDecoder(), 32)),
    ('nullifier1', fixDecoderSize(getBytesDecoder(), 32)),
    ('root', fixDecoderSize(getBytesDecoder(), 32)),
    ('relayerFee', getU64Decoder()),
    ('out0Commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('out0ViewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('out0EnvelopeLen', getU8Decoder()),
    ('out0Envelope', fixDecoderSize(getBytesDecoder(), 96)),
    ('out1Commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('out1ViewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('out1EnvelopeLen', getU8Decoder()),
    ('out1Envelope', fixDecoderSize(getBytesDecoder(), 96)),
    ('proofA', fixDecoderSize(getBytesDecoder(), 32)),
    ('proofB', fixDecoderSize(getBytesDecoder(), 128)),
    ('proofC', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'transfer instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TransferInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(6)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      TransferInstructionData(
        nullifier0Bump: map['nullifier0Bump']! as int,
        nullifier1Bump: map['nullifier1Bump']! as int,
        nullifier0: map['nullifier0']! as Uint8List,
        nullifier1: map['nullifier1']! as Uint8List,
        root: map['root']! as Uint8List,
        relayerFee: map['relayerFee']! as BigInt,
        out0Commitment: map['out0Commitment']! as Uint8List,
        out0ViewPubkey: map['out0ViewPubkey']! as Uint8List,
        out0EnvelopeLen: map['out0EnvelopeLen']! as int,
        out0Envelope: map['out0Envelope']! as Uint8List,
        out1Commitment: map['out1Commitment']! as Uint8List,
        out1ViewPubkey: map['out1ViewPubkey']! as Uint8List,
        out1EnvelopeLen: map['out1EnvelopeLen']! as int,
        out1Envelope: map['out1Envelope']! as Uint8List,
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

Codec<TransferInstructionData, TransferInstructionData>
getTransferInstructionDataCodec() {
  return combineCodec(
    getTransferInstructionDataEncoder(),
    getTransferInstructionDataDecoder(),
  );
}

/// Creates a [Transfer] instruction.
Instruction getTransferInstruction({
  required Address programAddress,
  required Address payer,
  required Address poolConfig,
  required Address poolVault,
  required Address merkleTree,
  required Address nullifierRecord0,
  required Address nullifierRecord1,
  required Address verifyingKeyAccount,
  required Address policyCosigner,
  required Address systemProgram,
  required int nullifier0Bump,
  required int nullifier1Bump,
  required Uint8List nullifier0,
  required Uint8List nullifier1,
  required Uint8List root,
  required BigInt relayerFee,
  required Uint8List out0Commitment,
  required Uint8List out0ViewPubkey,
  required int out0EnvelopeLen,
  required Uint8List out0Envelope,
  required Uint8List out1Commitment,
  required Uint8List out1ViewPubkey,
  required int out1EnvelopeLen,
  required Uint8List out1Envelope,
  required Uint8List proofA,
  required Uint8List proofB,
  required Uint8List proofC,
}) {
  final instructionData = TransferInstructionData(
    nullifier0Bump: nullifier0Bump,
    nullifier1Bump: nullifier1Bump,
    nullifier0: nullifier0,
    nullifier1: nullifier1,
    root: root,
    relayerFee: relayerFee,
    out0Commitment: out0Commitment,
    out0ViewPubkey: out0ViewPubkey,
    out0EnvelopeLen: out0EnvelopeLen,
    out0Envelope: out0Envelope,
    out1Commitment: out1Commitment,
    out1ViewPubkey: out1ViewPubkey,
    out1EnvelopeLen: out1EnvelopeLen,
    out1Envelope: out1Envelope,
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
      AccountMeta(address: policyCosigner, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getTransferInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Transfer] instruction from raw instruction data.
TransferInstructionData parseTransferInstruction(Instruction instruction) {
  return getTransferInstructionDataDecoder().decode(instruction.data!);
}
