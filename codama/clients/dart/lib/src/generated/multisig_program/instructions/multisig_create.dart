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
class MultisigCreateInstructionData {
  const MultisigCreateInstructionData({
    required this.bump,
    required this.threshold,
    required this.timelock,
    required this.ttl,
    required this.memberPermissions,
    required this.configAuthority,
    required this.rentCollector,
  }) : discriminator = 2,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final int threshold;
  final int timelock;
  final int ttl;
  final Uint8List memberPermissions;
  final Address configAuthority;
  final Address rentCollector;
}

Encoder<MultisigCreateInstructionData>
getMultisigCreateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('threshold', getU16Encoder()),
    ('timelock', getU32Encoder()),
    ('ttl', getU32Encoder()),
    (
      'memberPermissions',
      fixEncoderSize(getBytesEncoder(), 16, allowTruncation: false),
    ),
    ('configAuthority', getAddressEncoder()),
    ('rentCollector', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (MultisigCreateInstructionData value) => <String, Object?>{
      'discriminator': 2,
      'migrationVersion': 0,
      'bump': value.bump,
      'threshold': value.threshold,
      'timelock': value.timelock,
      'ttl': value.ttl,
      'memberPermissions': value.memberPermissions,
      'configAuthority': value.configAuthority,
      'rentCollector': value.rentCollector,
    },
  );
}

Decoder<MultisigCreateInstructionData>
getMultisigCreateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('threshold', getU16Decoder()),
    ('timelock', getU32Decoder()),
    ('ttl', getU32Decoder()),
    ('memberPermissions', fixDecoderSize(getBytesDecoder(), 16)),
    ('configAuthority', getAddressDecoder()),
    ('rentCollector', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'multisigCreate instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (MultisigCreateInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      MultisigCreateInstructionData(
        bump: map['bump']! as int,
        threshold: map['threshold']! as int,
        timelock: map['timelock']! as int,
        ttl: map['ttl']! as int,
        memberPermissions: map['memberPermissions']! as Uint8List,
        configAuthority: map['configAuthority']! as Address,
        rentCollector: map['rentCollector']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<MultisigCreateInstructionData>(
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
      VariableSizeDecoder<MultisigCreateInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<MultisigCreateInstructionData, MultisigCreateInstructionData>
getMultisigCreateInstructionDataCodec() {
  return combineCodec(
    getMultisigCreateInstructionDataEncoder(),
    getMultisigCreateInstructionDataDecoder(),
  );
}

/// Creates a [MultisigCreate] instruction.
Instruction getMultisigCreateInstruction({
  required Address programAddress,
  required Address programConfig,
  required Address createKey,
  required Address multisig,
  required Address rentPayer,
  required Address systemProgram,
  Address? treasury,
  required Address memberAccounts,
  required int bump,
  required int threshold,
  required int timelock,
  required int ttl,
  required Uint8List memberPermissions,
  required Address configAuthority,
  required Address rentCollector,
}) {
  final instructionData = MultisigCreateInstructionData(
    bump: bump,
    threshold: threshold,
    timelock: timelock,
    ttl: ttl,
    memberPermissions: memberPermissions,
    configAuthority: configAuthority,
    rentCollector: rentCollector,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: programConfig, role: AccountRole.readonly),
      AccountMeta(address: createKey, role: AccountRole.readonlySigner),
      AccountMeta(address: multisig, role: AccountRole.writable),
      AccountMeta(address: rentPayer, role: AccountRole.writableSigner),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      if (treasury != null)
        AccountMeta(address: treasury, role: AccountRole.writable)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
      AccountMeta(address: memberAccounts, role: AccountRole.readonly),
    ],
    data: getMultisigCreateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [MultisigCreate] instruction from raw instruction data.
MultisigCreateInstructionData parseMultisigCreateInstruction(
  Instruction instruction,
) {
  return getMultisigCreateInstructionDataDecoder().decode(instruction.data!);
}
