// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the TodoProgram program.
const todoProgramProgramAddress = Address(
  'Fc5A5xvNQ6w7kn2P7FpC18JNpDutLCRa14Q6gttxyPjd',
);

/// Known accounts for the TodoProgram program.
enum TodoProgramAccount { todoState }

/// Known instructions for the TodoProgram program.
enum TodoProgramInstruction { initialize, toggleCompleted, updateDigest }

/// Identifies the type of a TodoProgram instruction.
TodoProgramInstruction identifyTodoProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return TodoProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return TodoProgramInstruction.toggleCompleted;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return TodoProgramInstruction.updateDigest;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'todoProgram',
  });
}

/// A parsed instruction from the TodoProgram program.
sealed class ParsedTodoProgramInstruction {
  const ParsedTodoProgramInstruction(this.instructionType);

  final TodoProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedTodoProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(TodoProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed ToggleCompleted instruction.
final class ParsedToggleCompleted extends ParsedTodoProgramInstruction {
  const ParsedToggleCompleted({required this.data})
    : super(TodoProgramInstruction.toggleCompleted);

  final ToggleCompletedInstructionData data;
}

/// A parsed UpdateDigest instruction.
final class ParsedUpdateDigest extends ParsedTodoProgramInstruction {
  const ParsedUpdateDigest({required this.data})
    : super(TodoProgramInstruction.updateDigest);

  final UpdateDigestInstructionData data;
}

/// Parses a TodoProgram instruction.
ParsedTodoProgramInstruction parseTodoProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyTodoProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    TodoProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    TodoProgramInstruction.toggleCompleted => ParsedToggleCompleted(
      data: parseToggleCompletedInstruction(instruction),
    ),
    TodoProgramInstruction.updateDigest => ParsedUpdateDigest(
      data: parseUpdateDigestInstruction(instruction),
    ),
  };
}
