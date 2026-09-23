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
class ProposalCreateInstructionData {
  const ProposalCreateInstructionData({
    required this.bump,
    required this.kind,
    required this.vaultIndex,
    required this.vaultBump,
    required this.ephemeralSigners,
    required this.ephemeralBumps,
    required this.messageLen,
    required this.message,
    required this.actionsLen,
    required this.actions,
  }) :
      discriminator = 4,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final int kind;
  final int vaultIndex;
  final int vaultBump;
  final int ephemeralSigners;
  final Uint8List ephemeralBumps;
  final int messageLen;
  final Uint8List message;
  final int actionsLen;
  final Uint8List actions;
}

Encoder<ProposalCreateInstructionData> getProposalCreateInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('kind', getU8Encoder()),
    ('vaultIndex', getU8Encoder()),
    ('vaultBump', getU8Encoder()),
    ('ephemeralSigners', getU8Encoder()),
    ('ephemeralBumps', fixEncoderSize(getBytesEncoder(), 4, allowTruncation: false)),
    ('messageLen', getU16Encoder()),
    ('message', fixEncoderSize(getBytesEncoder(), 640, allowTruncation: false)),
    ('actionsLen', getU16Encoder()),
    ('actions', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (ProposalCreateInstructionData value) => <String, Object?>{
      'discriminator': 4,
      'migrationVersion': 0,
      'bump': value.bump,
      'kind': value.kind,
      'vaultIndex': value.vaultIndex,
      'vaultBump': value.vaultBump,
      'ephemeralSigners': value.ephemeralSigners,
      'ephemeralBumps': value.ephemeralBumps,
      'messageLen': value.messageLen,
      'message': value.message,
      'actionsLen': value.actionsLen,
      'actions': value.actions,
    },
  );
}

Decoder<ProposalCreateInstructionData> getProposalCreateInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('kind', getU8Decoder()),
    ('vaultIndex', getU8Decoder()),
    ('vaultBump', getU8Decoder()),
    ('ephemeralSigners', getU8Decoder()),
    ('ephemeralBumps', fixDecoderSize(getBytesDecoder(), 4)),
    ('messageLen', getU16Decoder()),
    ('message', fixDecoderSize(getBytesDecoder(), 640)),
    ('actionsLen', getU16Decoder()),
    ('actions', fixDecoderSize(getBytesDecoder(), 128)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'proposalCreate instruction decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (ProposalCreateInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(4),
    ).read(bytes, offset + 0);
    getConstantDecoder(
      getU8Encoder().encode(0),
    ).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      ProposalCreateInstructionData(
      bump: map['bump']! as int,
      kind: map['kind']! as int,
      vaultIndex: map['vaultIndex']! as int,
      vaultBump: map['vaultBump']! as int,
      ephemeralSigners: map['ephemeralSigners']! as int,
      ephemeralBumps: map['ephemeralBumps']! as Uint8List,
      messageLen: map['messageLen']! as int,
      message: map['message']! as Uint8List,
      actionsLen: map['actionsLen']! as int,
      actions: map['actions']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ProposalCreateInstructionData>(
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
      VariableSizeDecoder<ProposalCreateInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ProposalCreateInstructionData, ProposalCreateInstructionData> getProposalCreateInstructionDataCodec() {
  return combineCodec(getProposalCreateInstructionDataEncoder(), getProposalCreateInstructionDataDecoder());
}

/// Creates a [ProposalCreate] instruction.
Instruction getProposalCreateInstruction({
  required Address programAddress,
  required Address multisig,
  required Address proposal,
  required Address creator,
  required Address rentPayer,
  required Address systemProgram,
  required Address clock,
  required int bump,
  required int kind,
  required int vaultIndex,
  required int vaultBump,
  required int ephemeralSigners,
  required Uint8List ephemeralBumps,
  required int messageLen,
  required Uint8List message,
  required int actionsLen,
  required Uint8List actions,
}) {
  final instructionData = ProposalCreateInstructionData(
      bump: bump,
      kind: kind,
      vaultIndex: vaultIndex,
      vaultBump: vaultBump,
      ephemeralSigners: ephemeralSigners,
      ephemeralBumps: ephemeralBumps,
      messageLen: messageLen,
      message: message,
      actionsLen: actionsLen,
      actions: actions,
  );

  return Instruction(
    programAddress: programAddress,
    accounts: [
    AccountMeta(address: multisig, role: AccountRole.writable),
    AccountMeta(address: proposal, role: AccountRole.writable),
    AccountMeta(address: creator, role: AccountRole.readonlySigner),
    AccountMeta(address: rentPayer, role: AccountRole.writableSigner),
    AccountMeta(address: systemProgram, role: AccountRole.readonly),
    AccountMeta(address: clock, role: AccountRole.readonly),
    ],
    data: getProposalCreateInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ProposalCreate] instruction from raw instruction data.
ProposalCreateInstructionData parseProposalCreateInstruction(Instruction instruction) {
  return getProposalCreateInstructionDataDecoder().decode(instruction.data!);
}
