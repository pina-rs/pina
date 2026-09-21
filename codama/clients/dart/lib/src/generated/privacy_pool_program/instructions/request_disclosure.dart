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
class RequestDisclosureInstructionData {
  const RequestDisclosureInstructionData({
    required this.bump,
    required this.nonce,
    required this.tier,
    required this.commitment,
    required this.noticeLen,
    required this.notice,
    required this.legalBasisHash,
  }) :
      discriminator = 7,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final BigInt nonce;
  final int tier;
  final Uint8List commitment;
  final int noticeLen;
  final Uint8List notice;
  final Uint8List legalBasisHash;
}

Encoder<RequestDisclosureInstructionData> getRequestDisclosureInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('nonce', getU64Encoder()),
    ('tier', getU8Encoder()),
    ('commitment', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('noticeLen', getU8Encoder()),
    ('notice', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
    ('legalBasisHash', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (RequestDisclosureInstructionData value) => <String, Object?>{
      'discriminator': 7,
      'migrationVersion': 0,
      'bump': value.bump,
      'nonce': value.nonce,
      'tier': value.tier,
      'commitment': value.commitment,
      'noticeLen': value.noticeLen,
      'notice': value.notice,
      'legalBasisHash': value.legalBasisHash,
    },
  );
}

Decoder<RequestDisclosureInstructionData> getRequestDisclosureInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('nonce', getU64Decoder()),
    ('tier', getU8Decoder()),
    ('commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('noticeLen', getU8Decoder()),
    ('notice', fixDecoderSize(getBytesDecoder(), 96)),
    ('legalBasisHash', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'requestDisclosure instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RequestDisclosureInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(7),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      RequestDisclosureInstructionData(
      bump: map['bump']! as int,
      nonce: map['nonce']! as BigInt,
      tier: map['tier']! as int,
      commitment: map['commitment']! as Uint8List,
      noticeLen: map['noticeLen']! as int,
      notice: map['notice']! as Uint8List,
      legalBasisHash: map['legalBasisHash']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequestDisclosureInstructionData>(
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
      VariableSizeDecoder<RequestDisclosureInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequestDisclosureInstructionData, RequestDisclosureInstructionData> getRequestDisclosureInstructionDataCodec() {
  return combineCodec(getRequestDisclosureInstructionDataEncoder(), getRequestDisclosureInstructionDataDecoder());
}

/// Creates a [RequestDisclosure] instruction.
Instruction getRequestDisclosureInstruction({
  required Address programAddress,
  required Address requester,
  required Address poolConfig,
  required Address requesterRegistry,
  required Address noteCommitment,
  required Address disclosureRequest,
  required Address systemProgram,
  required Address clock,
  required int bump,
  required BigInt nonce,
  required int tier,
  required Uint8List commitment,
  required int noticeLen,
  required Uint8List notice,
  required Uint8List legalBasisHash,
}) {
  final instructionData = RequestDisclosureInstructionData(
      bump: bump,
      nonce: nonce,
      tier: tier,
      commitment: commitment,
      noticeLen: noticeLen,
      notice: notice,
      legalBasisHash: legalBasisHash,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: requester, role: AccountRole.writableSigner),
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: requesterRegistry, role: AccountRole.readonly),
    AccountMeta(address: noteCommitment, role: AccountRole.readonly),
    AccountMeta(address: disclosureRequest, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    AccountMeta(address: clock, role: AccountRole.readonly),
    ],
    data: getRequestDisclosureInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RequestDisclosure] instruction from raw instruction data.
RequestDisclosureInstructionData parseRequestDisclosureInstruction(Instruction instruction) {
  return getRequestDisclosureInstructionDataDecoder().decode(instruction.data!);
}
