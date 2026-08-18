// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorRealloc program.
const anchorReallocProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the AnchorRealloc program.
enum AnchorReallocInstruction { realloc, realloc2 }

/// Identifies the type of a AnchorRealloc instruction.
AnchorReallocInstruction identifyAnchorReallocInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorReallocInstruction.realloc;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AnchorReallocInstruction.realloc2;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorRealloc',
  });
}

/// A parsed instruction from the AnchorRealloc program.
sealed class ParsedAnchorReallocInstruction {
  const ParsedAnchorReallocInstruction(this.instructionType);

  final AnchorReallocInstruction instructionType;
}

/// A parsed Realloc instruction.
final class ParsedRealloc extends ParsedAnchorReallocInstruction {
  const ParsedRealloc({required this.data})
    : super(AnchorReallocInstruction.realloc);

  final ReallocInstructionData data;
}

/// A parsed Realloc2 instruction.
final class ParsedRealloc2 extends ParsedAnchorReallocInstruction {
  const ParsedRealloc2({required this.data})
    : super(AnchorReallocInstruction.realloc2);

  final Realloc2InstructionData data;
}

/// Parses a AnchorRealloc instruction.
ParsedAnchorReallocInstruction parseAnchorReallocInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorReallocInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorReallocInstruction.realloc => ParsedRealloc(
      data: parseReallocInstruction(instruction),
    ),
    AnchorReallocInstruction.realloc2 => ParsedRealloc2(
      data: parseRealloc2Instruction(instruction),
    ),
  };
}
