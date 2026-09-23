// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the HeapAllocProgram program.
const heapAllocProgramProgramAddress = Address('BZtBGtYSgERx2zN12aGgQ6sNLh5r7XtcNCafqmsizgqt');

/// Known instructions for the HeapAllocProgram program.
enum HeapAllocProgramInstruction {
  allocate,
  fill,
}

/// Identifies the type of a HeapAllocProgram instruction.
HeapAllocProgramInstruction identifyHeapAllocProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return HeapAllocProgramInstruction.allocate;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return HeapAllocProgramInstruction.fill;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'heapAllocProgram',
    },
  );
}

/// A parsed instruction from the HeapAllocProgram program.
sealed class ParsedHeapAllocProgramInstruction {
  const ParsedHeapAllocProgramInstruction(this.instructionType);

  final HeapAllocProgramInstruction instructionType;
}

/// A parsed Allocate instruction.
final class ParsedAllocate extends ParsedHeapAllocProgramInstruction {
  const ParsedAllocate({required this.data})
      : super(HeapAllocProgramInstruction.allocate);

  final AllocateInstructionData data;
}

/// A parsed Fill instruction.
final class ParsedFill extends ParsedHeapAllocProgramInstruction {
  const ParsedFill({required this.data})
      : super(HeapAllocProgramInstruction.fill);

  final FillInstructionData data;
}

/// Parses a HeapAllocProgram instruction.
ParsedHeapAllocProgramInstruction parseHeapAllocProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyHeapAllocProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    HeapAllocProgramInstruction.allocate => ParsedAllocate(
      data: parseAllocateInstruction(instruction),
    ),
    HeapAllocProgramInstruction.fill => ParsedFill(
      data: parseFillInstruction(instruction),
    ),
  };
}
