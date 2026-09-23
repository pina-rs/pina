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
class SetVerificationKeyInstructionData {
  const SetVerificationKeyInstructionData({
    required this.bump,
    required this.slot,
    required this.icLen,
    required this.alphaG1,
    required this.betaG2,
    required this.gammaG2,
    required this.deltaG2,
    required this.ic0,
    required this.ic1,
    required this.ic2,
    required this.ic3,
  }) :
      discriminator = 1,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final int slot;
  final int icLen;
  final Uint8List alphaG1;
  final Uint8List betaG2;
  final Uint8List gammaG2;
  final Uint8List deltaG2;
  final Uint8List ic0;
  final Uint8List ic1;
  final Uint8List ic2;
  final Uint8List ic3;
}

Encoder<SetVerificationKeyInstructionData> getSetVerificationKeyInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('slot', getU8Encoder()),
    ('icLen', getU8Encoder()),
    ('alphaG1', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('betaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('gammaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('deltaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('ic0', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic1', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic2', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic3', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (SetVerificationKeyInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'bump': value.bump,
      'slot': value.slot,
      'icLen': value.icLen,
      'alphaG1': value.alphaG1,
      'betaG2': value.betaG2,
      'gammaG2': value.gammaG2,
      'deltaG2': value.deltaG2,
      'ic0': value.ic0,
      'ic1': value.ic1,
      'ic2': value.ic2,
      'ic3': value.ic3,
    },
  );
}

Decoder<SetVerificationKeyInstructionData> getSetVerificationKeyInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('slot', getU8Decoder()),
    ('icLen', getU8Decoder()),
    ('alphaG1', fixDecoderSize(getBytesDecoder(), 64)),
    ('betaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('gammaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('deltaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('ic0', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic1', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic2', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic3', fixDecoderSize(getBytesDecoder(), 64)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'setVerificationKey instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (SetVerificationKeyInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(1),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      SetVerificationKeyInstructionData(
      bump: map['bump']! as int,
      slot: map['slot']! as int,
      icLen: map['icLen']! as int,
      alphaG1: map['alphaG1']! as Uint8List,
      betaG2: map['betaG2']! as Uint8List,
      gammaG2: map['gammaG2']! as Uint8List,
      deltaG2: map['deltaG2']! as Uint8List,
      ic0: map['ic0']! as Uint8List,
      ic1: map['ic1']! as Uint8List,
      ic2: map['ic2']! as Uint8List,
      ic3: map['ic3']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SetVerificationKeyInstructionData>(
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
      VariableSizeDecoder<SetVerificationKeyInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SetVerificationKeyInstructionData, SetVerificationKeyInstructionData> getSetVerificationKeyInstructionDataCodec() {
  return combineCodec(getSetVerificationKeyInstructionDataEncoder(), getSetVerificationKeyInstructionDataDecoder());
}

/// Creates a [SetVerificationKey] instruction.
Instruction getSetVerificationKeyInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required Address verifyingKeyAccount,
  required Address systemProgram,
  required int bump,
  required int slot,
  required int icLen,
  required Uint8List alphaG1,
  required Uint8List betaG2,
  required Uint8List gammaG2,
  required Uint8List deltaG2,
  required Uint8List ic0,
  required Uint8List ic1,
  required Uint8List ic2,
  required Uint8List ic3,
}) {
  final instructionData = SetVerificationKeyInstructionData(
      bump: bump,
      slot: slot,
      icLen: icLen,
      alphaG1: alphaG1,
      betaG2: betaG2,
      gammaG2: gammaG2,
      deltaG2: deltaG2,
      ic0: ic0,
      ic1: ic1,
      ic2: ic2,
      ic3: ic3,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.writableSigner),
    AccountMeta(address: poolConfig, role: AccountRole.readonly),
    AccountMeta(address: verifyingKeyAccount, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getSetVerificationKeyInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [SetVerificationKey] instruction from raw instruction data.
SetVerificationKeyInstructionData parseSetVerificationKeyInstruction(Instruction instruction) {
  return getSetVerificationKeyInstructionDataDecoder().decode(instruction.data!);
}
