// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AccountReallocProgram program.
const accountReallocProgramProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known accounts for the AccountReallocProgram program.
enum AccountReallocProgramAccount { sample }

/// Known instructions for the AccountReallocProgram program.
enum AccountReallocProgramInstruction { initialize, realloc, realloc2 }

/// Identifies the type of a AccountReallocProgram instruction.
AccountReallocProgramInstruction identifyAccountReallocProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return AccountReallocProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AccountReallocProgramInstruction.realloc;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AccountReallocProgramInstruction.realloc2;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'accountReallocProgram',
  });
}

/// A parsed instruction from the AccountReallocProgram program.
sealed class ParsedAccountReallocProgramInstruction {
  const ParsedAccountReallocProgramInstruction(this.instructionType);

  final AccountReallocProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedAccountReallocProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(AccountReallocProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Realloc instruction.
final class ParsedRealloc extends ParsedAccountReallocProgramInstruction {
  const ParsedRealloc({required this.data})
    : super(AccountReallocProgramInstruction.realloc);

  final ReallocInstructionData data;
}

/// A parsed Realloc2 instruction.
final class ParsedRealloc2 extends ParsedAccountReallocProgramInstruction {
  const ParsedRealloc2({required this.data})
    : super(AccountReallocProgramInstruction.realloc2);

  final Realloc2InstructionData data;
}

/// Parses a AccountReallocProgram instruction.
ParsedAccountReallocProgramInstruction parseAccountReallocProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyAccountReallocProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AccountReallocProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    AccountReallocProgramInstruction.realloc => ParsedRealloc(
      data: parseReallocInstruction(instruction),
    ),
    AccountReallocProgramInstruction.realloc2 => ParsedRealloc2(
      data: parseRealloc2Instruction(instruction),
    ),
  };
}
