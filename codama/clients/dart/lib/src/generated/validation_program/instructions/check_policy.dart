// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';


@immutable
class CheckPolicyInstructionData {
  const CheckPolicyInstructionData({
    required this.amount,
    required this.memo,
    required this.approvals,
  }) :
      discriminator = 1;

  final int discriminator;
  final BigInt amount;
  final String memo;
  final List<int> approvals;
}

Encoder<CheckPolicyInstructionData> getCheckPolicyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('amount', getU64Encoder()),
    ('memo', fixEncoderSize(addEncoderSizePrefix(getUtf8Encoder(), getU8Encoder()), 65, allowTruncation: false)),
    ('approvals', fixEncoderSize(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(getU16Encoder())), 6, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (CheckPolicyInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'amount': value.amount,
      'memo': value.memo,
      'approvals': value.approvals,
    },
  );
}

Decoder<CheckPolicyInstructionData> getCheckPolicyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('amount', getU64Decoder()),
    ('memo', fixDecoderSize(addDecoderSizePrefix(getUtf8Decoder(), getU8Decoder()), 65)),
    ('approvals', fixDecoderSize(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(getU16Decoder())), 6)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'checkPolicy instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (CheckPolicyInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(1),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      CheckPolicyInstructionData(
      amount: map['amount']! as BigInt,
      memo: map['memo']! as String,
      approvals: map['approvals']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CheckPolicyInstructionData>(
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
      VariableSizeDecoder<CheckPolicyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CheckPolicyInstructionData, CheckPolicyInstructionData> getCheckPolicyInstructionDataCodec() {
  return combineCodec(getCheckPolicyInstructionDataEncoder(), getCheckPolicyInstructionDataDecoder());
}

/// Creates a [CheckPolicy] instruction.
Instruction getCheckPolicyInstruction({
  required Address programAddress,
  required Address authority,
  required Address policy,
  required Address audit,
  required Address systemProgram,
  required BigInt amount,
  required String memo,
  required List<int> approvals,
}) {
  final instructionData = CheckPolicyInstructionData(
      amount: amount,
      memo: memo,
      approvals: approvals,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.readonlySigner),
    AccountMeta(address: policy, role: AccountRole.readonly),
    AccountMeta(address: audit, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getCheckPolicyInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [CheckPolicy] instruction from raw instruction data.
CheckPolicyInstructionData parseCheckPolicyInstruction(Instruction instruction) {
  return getCheckPolicyInstructionDataDecoder().decode(instruction.data!);
}
