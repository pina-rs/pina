// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the DeclareId program.
const declareIdProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the DeclareId program.
enum DeclareIdInstruction { initialize }

/// Identifies the type of a DeclareId instruction.
DeclareIdInstruction identifyDeclareIdInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return DeclareIdInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'declareId',
  });
}

/// A parsed instruction from the DeclareId program.
sealed class ParsedDeclareIdInstruction {
  const ParsedDeclareIdInstruction(this.instructionType);

  final DeclareIdInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedDeclareIdInstruction {
  const ParsedInitialize({required this.data})
    : super(DeclareIdInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a DeclareId instruction.
ParsedDeclareIdInstruction parseDeclareIdInstruction(Instruction instruction) {
  return switch (identifyDeclareIdInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    DeclareIdInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
