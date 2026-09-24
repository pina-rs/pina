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
class InitializeInstructionData {
  const InitializeInstructionData({
    required this.configBump,
    required this.vaultBump,
    required this.treeBump,
    required this.nullifiersBump,
    required this.custodiansBump,
    required this.requestersBump,
    required this.logBump,
    required this.custodians,
  }) :
      discriminator = 0,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int configBump;
  final int vaultBump;
  final int treeBump;
  final int nullifiersBump;
  final int custodiansBump;
  final int requestersBump;
  final int logBump;
  final Uint8List custodians;
}

Encoder<InitializeInstructionData> getInitializeInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('configBump', getU8Encoder()),
    ('vaultBump', getU8Encoder()),
    ('treeBump', getU8Encoder()),
    ('nullifiersBump', getU8Encoder()),
    ('custodiansBump', getU8Encoder()),
    ('requestersBump', getU8Encoder()),
    ('logBump', getU8Encoder()),
    ('custodians', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializeInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'migrationVersion': 0,
      'configBump': value.configBump,
      'vaultBump': value.vaultBump,
      'treeBump': value.treeBump,
      'nullifiersBump': value.nullifiersBump,
      'custodiansBump': value.custodiansBump,
      'requestersBump': value.requestersBump,
      'logBump': value.logBump,
      'custodians': value.custodians,
    },
  );
}

Decoder<InitializeInstructionData> getInitializeInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('configBump', getU8Decoder()),
    ('vaultBump', getU8Decoder()),
    ('treeBump', getU8Decoder()),
    ('nullifiersBump', getU8Decoder()),
    ('custodiansBump', getU8Decoder()),
    ('requestersBump', getU8Decoder()),
    ('logBump', getU8Decoder()),
    ('custodians', fixDecoderSize(getBytesDecoder(), 96)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'initialize instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (InitializeInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      InitializeInstructionData(
      configBump: map['configBump']! as int,
      vaultBump: map['vaultBump']! as int,
      treeBump: map['treeBump']! as int,
      nullifiersBump: map['nullifiersBump']! as int,
      custodiansBump: map['custodiansBump']! as int,
      requestersBump: map['requestersBump']! as int,
      logBump: map['logBump']! as int,
      custodians: map['custodians']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitializeInstructionData>(
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
      VariableSizeDecoder<InitializeInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitializeInstructionData, InitializeInstructionData> getInitializeInstructionDataCodec() {
  return combineCodec(getInitializeInstructionDataEncoder(), getInitializeInstructionDataDecoder());
}

/// Creates a [Initialize] instruction.
Instruction getInitializeInstruction({
  required Address programAddress,
  required Address authority,
  required Address poolConfig,
  required Address poolVault,
  required Address merkleTree,
  required Address nullifierSet,
  required Address custodianRegistry,
  required Address requesterRegistry,
  required Address disclosureLog,
  required Address systemProgram,
  required int configBump,
  required int vaultBump,
  required int treeBump,
  required int nullifiersBump,
  required int custodiansBump,
  required int requestersBump,
  required int logBump,
  required Uint8List custodians,
}) {
  final instructionData = InitializeInstructionData(
      configBump: configBump,
      vaultBump: vaultBump,
      treeBump: treeBump,
      nullifiersBump: nullifiersBump,
      custodiansBump: custodiansBump,
      requestersBump: requestersBump,
      logBump: logBump,
      custodians: custodians,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: authority, role: AccountRole.writableSigner),
    AccountMeta(address: poolConfig, role: AccountRole.writable),
    AccountMeta(address: poolVault, role: AccountRole.writable),
    AccountMeta(address: merkleTree, role: AccountRole.writable),
    AccountMeta(address: nullifierSet, role: AccountRole.writable),
    AccountMeta(address: custodianRegistry, role: AccountRole.writable),
    AccountMeta(address: requesterRegistry, role: AccountRole.writable),
    AccountMeta(address: disclosureLog, role: AccountRole.writable),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getInitializeInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Initialize] instruction from raw instruction data.
InitializeInstructionData parseInitializeInstruction(Instruction instruction) {
  return getInitializeInstructionDataDecoder().decode(instruction.data!);
}
