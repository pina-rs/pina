// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the CompactAccounts program.
const compactAccountsProgramAddress = Address(
  '85qGHkkBAdE61PZSNF9R6UYakqw8d5eonqi4jbFLaSTn',
);

/// Known accounts for the CompactAccounts program.
enum CompactAccountsAccount { journal }

/// Known instructions for the CompactAccounts program.
enum CompactAccountsInstruction { initialize, resize, write, rename }

/// Identifies the type of a CompactAccounts instruction.
CompactAccountsInstruction identifyCompactAccountsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return CompactAccountsInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return CompactAccountsInstruction.resize;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return CompactAccountsInstruction.write;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return CompactAccountsInstruction.rename;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'compactAccounts',
  });
}

/// A parsed instruction from the CompactAccounts program.
sealed class ParsedCompactAccountsInstruction {
  const ParsedCompactAccountsInstruction(this.instructionType);

  final CompactAccountsInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedCompactAccountsInstruction {
  const ParsedInitialize({required this.data})
    : super(CompactAccountsInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Resize instruction.
final class ParsedResize extends ParsedCompactAccountsInstruction {
  const ParsedResize({required this.data})
    : super(CompactAccountsInstruction.resize);

  final ResizeInstructionData data;
}

/// A parsed Write instruction.
final class ParsedWrite extends ParsedCompactAccountsInstruction {
  const ParsedWrite({required this.data})
    : super(CompactAccountsInstruction.write);

  final WriteInstructionData data;
}

/// A parsed Rename instruction.
final class ParsedRename extends ParsedCompactAccountsInstruction {
  const ParsedRename({required this.data})
    : super(CompactAccountsInstruction.rename);

  final RenameInstructionData data;
}

/// Parses a CompactAccounts instruction.
ParsedCompactAccountsInstruction parseCompactAccountsInstruction(
  Instruction instruction,
) {
  return switch (identifyCompactAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    CompactAccountsInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    CompactAccountsInstruction.resize => ParsedResize(
      data: parseResizeInstruction(instruction),
    ),
    CompactAccountsInstruction.write => ParsedWrite(
      data: parseWriteInstruction(instruction),
    ),
    CompactAccountsInstruction.rename => ParsedRename(
      data: parseRenameInstruction(instruction),
    ),
  };
}
