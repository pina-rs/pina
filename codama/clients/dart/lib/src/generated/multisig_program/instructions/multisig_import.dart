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
class MultisigImportInstructionData {
  const MultisigImportInstructionData({
    required this.bump,
    required this.legacyProgram,
    required this.legacyDiscriminator,
    required this.setConfigAuthority,
    required this.configAuthority,
    required this.setRentCollector,
    required this.rentCollector,
  }) :
      discriminator = 3,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address legacyProgram;
  final Uint8List legacyDiscriminator;
  final bool setConfigAuthority;
  final Address configAuthority;
  final bool setRentCollector;
  final Address rentCollector;
}

Encoder<MultisigImportInstructionData> getMultisigImportInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('legacyProgram', getAddressEncoder()),
    ('legacyDiscriminator', fixEncoderSize(getBytesEncoder(), 8, allowTruncation: false)),
    ('setConfigAuthority', getBooleanEncoder()),
    ('configAuthority', getAddressEncoder()),
    ('setRentCollector', getBooleanEncoder()),
    ('rentCollector', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (MultisigImportInstructionData value) => <String, Object?>{
      'discriminator': 3,
      'migrationVersion': 0,
      'bump': value.bump,
      'legacyProgram': value.legacyProgram,
      'legacyDiscriminator': value.legacyDiscriminator,
      'setConfigAuthority': value.setConfigAuthority,
      'configAuthority': value.configAuthority,
      'setRentCollector': value.setRentCollector,
      'rentCollector': value.rentCollector,
    },
  );
}

Decoder<MultisigImportInstructionData> getMultisigImportInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('legacyProgram', getAddressDecoder()),
    ('legacyDiscriminator', fixDecoderSize(getBytesDecoder(), 8)),
    ('setConfigAuthority', getBooleanDecoder()),
    ('configAuthority', getAddressDecoder()),
    ('setRentCollector', getBooleanDecoder()),
    ('rentCollector', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'multisigImport instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (MultisigImportInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(3),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      MultisigImportInstructionData(
      bump: map['bump']! as int,
      legacyProgram: map['legacyProgram']! as Address,
      legacyDiscriminator: map['legacyDiscriminator']! as Uint8List,
      setConfigAuthority: map['setConfigAuthority']! as bool,
      configAuthority: map['configAuthority']! as Address,
      setRentCollector: map['setRentCollector']! as bool,
      rentCollector: map['rentCollector']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<MultisigImportInstructionData>(
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
      VariableSizeDecoder<MultisigImportInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<MultisigImportInstructionData, MultisigImportInstructionData> getMultisigImportInstructionDataCodec() {
  return combineCodec(getMultisigImportInstructionDataEncoder(), getMultisigImportInstructionDataDecoder());
}

/// Creates a [MultisigImport] instruction.
Instruction getMultisigImportInstruction({
  required Address programAddress,
  required Address legacyMultisig,
  required Address programConfig,
  required Address createKey,
  required Address multisig,
  required Address rentPayer,
  required Address systemProgram,
  Address? treasury,
  required int bump,
  required Address legacyProgram,
  required Uint8List legacyDiscriminator,
  required bool setConfigAuthority,
  required Address configAuthority,
  required bool setRentCollector,
  required Address rentCollector,
}) {
  final instructionData = MultisigImportInstructionData(
      bump: bump,
      legacyProgram: legacyProgram,
      legacyDiscriminator: legacyDiscriminator,
      setConfigAuthority: setConfigAuthority,
      configAuthority: configAuthority,
      setRentCollector: setRentCollector,
      rentCollector: rentCollector,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: legacyMultisig, role: AccountRole.readonly),
    AccountMeta(address: programConfig, role: AccountRole.readonly),
    AccountMeta(address: createKey, role: AccountRole.readonlySigner),
    AccountMeta(address: multisig, role: AccountRole.writable),
    AccountMeta(address: rentPayer, role: AccountRole.writableSigner),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    if (treasury != null) AccountMeta(address: treasury, role: AccountRole.writable) else AccountMeta(address: programAddress, role: AccountRole.readonly),
    ],
    data: getMultisigImportInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [MultisigImport] instruction from raw instruction data.
MultisigImportInstructionData parseMultisigImportInstruction(Instruction instruction) {
  return getMultisigImportInstructionDataDecoder().decode(instruction.data!);
}
