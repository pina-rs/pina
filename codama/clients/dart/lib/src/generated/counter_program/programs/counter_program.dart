// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the CounterProgram program.
const counterProgramProgramAddress = Address(
  'GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS',
);

/// Known accounts for the CounterProgram program.
enum CounterProgramAccount { counterState }

/// Known instructions for the CounterProgram program.
enum CounterProgramInstruction { initialize, increment }

/// Identifies the type of a CounterProgram instruction.
CounterProgramInstruction identifyCounterProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return CounterProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return CounterProgramInstruction.increment;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'counterProgram',
  });
}

/// A parsed instruction from the CounterProgram program.
sealed class ParsedCounterProgramInstruction {
  const ParsedCounterProgramInstruction(this.instructionType);

  final CounterProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedCounterProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(CounterProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Increment instruction.
final class ParsedIncrement extends ParsedCounterProgramInstruction {
  const ParsedIncrement({required this.data})
    : super(CounterProgramInstruction.increment);

  final IncrementInstructionData data;
}

/// Parses a CounterProgram instruction.
ParsedCounterProgramInstruction parseCounterProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyCounterProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    CounterProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    CounterProgramInstruction.increment => ParsedIncrement(
      data: parseIncrementInstruction(instruction),
    ),
  };
}
