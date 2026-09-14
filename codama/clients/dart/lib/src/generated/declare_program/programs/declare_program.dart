// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the DeclareProgram program.
const declareProgramProgramAddress = Address('Dec1areProgram11111111111111111111111111111');

/// Known instructions for the DeclareProgram program.
enum DeclareProgramInstruction {
  validateExternalProgram,
}

/// Identifies the type of a DeclareProgram instruction.
DeclareProgramInstruction identifyDeclareProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return DeclareProgramInstruction.validateExternalProgram;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'declareProgram',
    },
  );
}

/// A parsed instruction from the DeclareProgram program.
sealed class ParsedDeclareProgramInstruction {
  const ParsedDeclareProgramInstruction(this.instructionType);

  final DeclareProgramInstruction instructionType;
}

/// A parsed ValidateExternalProgram instruction.
final class ParsedValidateExternalProgram extends ParsedDeclareProgramInstruction {
  const ParsedValidateExternalProgram({required this.data})
      : super(DeclareProgramInstruction.validateExternalProgram);

  final ValidateExternalProgramInstructionData data;
}

/// Parses a DeclareProgram instruction.
ParsedDeclareProgramInstruction parseDeclareProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyDeclareProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    DeclareProgramInstruction.validateExternalProgram => ParsedValidateExternalProgram(
      data: parseValidateExternalProgramInstruction(instruction),
    ),
  };
}
