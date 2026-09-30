// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the SystemAccountsProgram program.
const systemAccountsProgramProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the SystemAccountsProgram program.
enum SystemAccountsProgramInstruction { initialize }

/// Identifies the type of a SystemAccountsProgram instruction.
SystemAccountsProgramInstruction identifySystemAccountsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return SystemAccountsProgramInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'systemAccountsProgram',
  });
}

/// A parsed instruction from the SystemAccountsProgram program.
sealed class ParsedSystemAccountsProgramInstruction {
  const ParsedSystemAccountsProgramInstruction(this.instructionType);

  final SystemAccountsProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedSystemAccountsProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(SystemAccountsProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a SystemAccountsProgram instruction.
ParsedSystemAccountsProgramInstruction parseSystemAccountsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifySystemAccountsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    SystemAccountsProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
