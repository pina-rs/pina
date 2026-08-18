// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorDeclareProgram program.
const anchorDeclareProgramProgramAddress = Address(
  'Dec1areProgram11111111111111111111111111111',
);

/// Known instructions for the AnchorDeclareProgram program.
enum AnchorDeclareProgramInstruction { validateExternalProgram }

/// Identifies the type of a AnchorDeclareProgram instruction.
AnchorDeclareProgramInstruction identifyAnchorDeclareProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorDeclareProgramInstruction.validateExternalProgram;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorDeclareProgram',
  });
}

/// A parsed instruction from the AnchorDeclareProgram program.
sealed class ParsedAnchorDeclareProgramInstruction {
  const ParsedAnchorDeclareProgramInstruction(this.instructionType);

  final AnchorDeclareProgramInstruction instructionType;
}

/// A parsed ValidateExternalProgram instruction.
final class ParsedValidateExternalProgram
    extends ParsedAnchorDeclareProgramInstruction {
  const ParsedValidateExternalProgram({required this.data})
    : super(AnchorDeclareProgramInstruction.validateExternalProgram);

  final ValidateExternalProgramInstructionData data;
}

/// Parses a AnchorDeclareProgram instruction.
ParsedAnchorDeclareProgramInstruction parseAnchorDeclareProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorDeclareProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorDeclareProgramInstruction.validateExternalProgram =>
      ParsedValidateExternalProgram(
        data: parseValidateExternalProgramInstruction(instruction),
      ),
  };
}
