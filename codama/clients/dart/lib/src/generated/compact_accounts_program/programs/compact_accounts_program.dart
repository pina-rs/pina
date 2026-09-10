// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the CompactAccountsProgram program.
const compactAccountsProgramProgramAddress = Address(
  '85qGHkkBAdE61PZSNF9R6UYakqw8d5eonqi4jbFLaSTn',
);

/// Known accounts for the CompactAccountsProgram program.
enum CompactAccountsProgramAccount { journal }

/// Known instructions for the CompactAccountsProgram program.
enum CompactAccountsProgramInstruction { initialize, resize, write, rename }

/// Identifies the type of a CompactAccountsProgram instruction.
CompactAccountsProgramInstruction identifyCompactAccountsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return CompactAccountsProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return CompactAccountsProgramInstruction.resize;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return CompactAccountsProgramInstruction.write;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return CompactAccountsProgramInstruction.rename;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'compactAccountsProgram',
  });
}

/// A parsed instruction from the CompactAccountsProgram program.
sealed class ParsedCompactAccountsProgramInstruction {
  const ParsedCompactAccountsProgramInstruction(this.instructionType);

  final CompactAccountsProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedCompactAccountsProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(CompactAccountsProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Resize instruction.
final class ParsedResize extends ParsedCompactAccountsProgramInstruction {
  const ParsedResize({required this.data})
    : super(CompactAccountsProgramInstruction.resize);

  final ResizeInstructionData data;
}

/// A parsed Write instruction.
final class ParsedWrite extends ParsedCompactAccountsProgramInstruction {
  const ParsedWrite({required this.data})
    : super(CompactAccountsProgramInstruction.write);

  final WriteInstructionData data;
}

/// A parsed Rename instruction.
final class ParsedRename extends ParsedCompactAccountsProgramInstruction {
  const ParsedRename({required this.data})
    : super(CompactAccountsProgramInstruction.rename);

  final RenameInstructionData data;
}

/// Parses a CompactAccountsProgram instruction.
ParsedCompactAccountsProgramInstruction parseCompactAccountsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyCompactAccountsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    CompactAccountsProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    CompactAccountsProgramInstruction.resize => ParsedResize(
      data: parseResizeInstruction(instruction),
    ),
    CompactAccountsProgramInstruction.write => ParsedWrite(
      data: parseWriteInstruction(instruction),
    ),
    CompactAccountsProgramInstruction.rename => ParsedRename(
      data: parseRenameInstruction(instruction),
    ),
  };
}
