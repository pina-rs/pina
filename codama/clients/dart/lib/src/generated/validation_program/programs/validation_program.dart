// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the ValidationProgram program.
const validationProgramProgramAddress = Address('GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn');

/// Known accounts for the ValidationProgram program.
enum ValidationProgramAccount {
  policyState,
}

/// Known instructions for the ValidationProgram program.
enum ValidationProgramInstruction {
  initializePolicy,
  checkPolicy,
}

/// Identifies the type of a ValidationProgram instruction.
ValidationProgramInstruction identifyValidationProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return ValidationProgramInstruction.initializePolicy;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return ValidationProgramInstruction.checkPolicy;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'validationProgram',
    },
  );
}

/// A parsed instruction from the ValidationProgram program.
sealed class ParsedValidationProgramInstruction {
  const ParsedValidationProgramInstruction(this.instructionType);

  final ValidationProgramInstruction instructionType;
}

/// A parsed InitializePolicy instruction.
final class ParsedInitializePolicy extends ParsedValidationProgramInstruction {
  const ParsedInitializePolicy({required this.data})
      : super(ValidationProgramInstruction.initializePolicy);

  final InitializePolicyInstructionData data;
}

/// A parsed CheckPolicy instruction.
final class ParsedCheckPolicy extends ParsedValidationProgramInstruction {
  const ParsedCheckPolicy({required this.data})
      : super(ValidationProgramInstruction.checkPolicy);

  final CheckPolicyInstructionData data;
}

/// Parses a ValidationProgram instruction.
ParsedValidationProgramInstruction parseValidationProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyValidationProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    ValidationProgramInstruction.initializePolicy => ParsedInitializePolicy(
      data: parseInitializePolicyInstruction(instruction),
    ),
    ValidationProgramInstruction.checkPolicy => ParsedCheckPolicy(
      data: parseCheckPolicyInstruction(instruction),
    ),
  };
}
