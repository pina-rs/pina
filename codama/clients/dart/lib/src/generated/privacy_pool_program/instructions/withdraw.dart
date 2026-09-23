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
    required this.nullifier,
    required this.root,
    required this.proofA,
    required this.proofB,
    required this.proofC,
  }) :
      discriminator = 5,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Uint8List nullifier;
  final Uint8List root;
  final Uint8List proofA;
  final Uint8List proofB;
  final Uint8List proofC;
}

Encoder<WithdrawInstructionData> getWithdrawInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('nullifier', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('root', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('proofA', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('proofB', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('proofC', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (WithdrawInstructionData value) => <String, Object?>{
      'discriminator': 5,
      'migrationVersion': 0,
      'nullifier': value.nullifier,
      'root': value.root,
      'proofA': value.proofA,
      'proofB': value.proofB,
      'proofC': value.proofC,
    },
  );
}

Decoder<WithdrawInstructionData> getWithdrawInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('nullifier', fixDecoderSize(getBytesDecoder(), 32)),
    ('root', fixDecoderSize(getBytesDecoder(), 32)),
    ('proofA', fixDecoderSize(getBytesDecoder(), 64)),
    ('proofB', fixDecoderSize(getBytesDecoder(), 128)),
    ('proofC', fixDecoderSize(getBytesDecoder(), 64)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'withdraw instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (WithdrawInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(5),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      WithdrawInstructionData(
      nullifier: map['nullifier']! as Uint8List,
      root: map['root']! as Uint8List,
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

Codec<WithdrawInstructionData, WithdrawInstructionData> getWithdrawInstructionDataCodec() {
  return combineCodec(getWithdrawInstructionDataEncoder(), getWithdrawInstructionDataDecoder());
}

/// Creates a [Withdraw] instruction.
Instruction getWithdrawInstruction({
  required Address programAddress,
  required Address poolConfig,
  required Address poolVault,
  required Address merkleTree,
  required Address nullifierSet,
  required Address verifyingKeyAccount,
  required Address recipient,
  required Address systemProgram,
  required Uint8List nullifier,
  required Uint8List root,
  required Uint8List proofA,
  required Uint8List proofB,
  required Uint8List proofC,
}) {
  final instructionData = WithdrawInstructionData(
      nullifier: nullifier,
      root: root,
      proofA: proofA,
      proofB: proofB,
      proofC: proofC,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: poolVault, role: AccountRole.writable),
    AccountMeta(address: merkleTree, role: AccountRole.readonly),
    AccountMeta(address: nullifierSet, role: AccountRole.writable),
    AccountMeta(address: verifyingKeyAccount, role: AccountRole.readonly),
    AccountMeta(address: recipient, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getWithdrawInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Withdraw] instruction from raw instruction data.
WithdrawInstructionData parseWithdrawInstruction(Instruction instruction) {
  return getWithdrawInstructionDataDecoder().decode(instruction.data!);
}
