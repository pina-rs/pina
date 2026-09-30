// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the DeclareIdProgram program.
const declareIdProgramProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the DeclareIdProgram program.
enum DeclareIdProgramInstruction { initialize }

/// Identifies the type of a DeclareIdProgram instruction.
DeclareIdProgramInstruction identifyDeclareIdProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return DeclareIdProgramInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'declareIdProgram',
  });
}

/// A parsed instruction from the DeclareIdProgram program.
sealed class ParsedDeclareIdProgramInstruction {
  const ParsedDeclareIdProgramInstruction(this.instructionType);

  final DeclareIdProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedDeclareIdProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(DeclareIdProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a DeclareIdProgram instruction.
ParsedDeclareIdProgramInstruction parseDeclareIdProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyDeclareIdProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    DeclareIdProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
