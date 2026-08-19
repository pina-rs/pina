// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorDeclareId program.
const anchorDeclareIdProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the AnchorDeclareId program.
enum AnchorDeclareIdInstruction { initialize }

/// Identifies the type of a AnchorDeclareId instruction.
AnchorDeclareIdInstruction identifyAnchorDeclareIdInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorDeclareIdInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorDeclareId',
  });
}

/// A parsed instruction from the AnchorDeclareId program.
sealed class ParsedAnchorDeclareIdInstruction {
  const ParsedAnchorDeclareIdInstruction(this.instructionType);

  final AnchorDeclareIdInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedAnchorDeclareIdInstruction {
  const ParsedInitialize({required this.data})
    : super(AnchorDeclareIdInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a AnchorDeclareId instruction.
ParsedAnchorDeclareIdInstruction parseAnchorDeclareIdInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorDeclareIdInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorDeclareIdInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
