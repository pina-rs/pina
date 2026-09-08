// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AccountRealloc program.
const accountReallocProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known accounts for the AccountRealloc program.
enum AccountReallocAccount { sample }

/// Known instructions for the AccountRealloc program.
enum AccountReallocInstruction { initialize, realloc, realloc2 }

/// Identifies the type of a AccountRealloc instruction.
AccountReallocInstruction identifyAccountReallocInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return AccountReallocInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AccountReallocInstruction.realloc;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AccountReallocInstruction.realloc2;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'accountRealloc',
  });
}

/// A parsed instruction from the AccountRealloc program.
sealed class ParsedAccountReallocInstruction {
  const ParsedAccountReallocInstruction(this.instructionType);

  final AccountReallocInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedAccountReallocInstruction {
  const ParsedInitialize({required this.data})
    : super(AccountReallocInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Realloc instruction.
final class ParsedRealloc extends ParsedAccountReallocInstruction {
  const ParsedRealloc({required this.data})
    : super(AccountReallocInstruction.realloc);

  final ReallocInstructionData data;
}

/// A parsed Realloc2 instruction.
final class ParsedRealloc2 extends ParsedAccountReallocInstruction {
  const ParsedRealloc2({required this.data})
    : super(AccountReallocInstruction.realloc2);

  final Realloc2InstructionData data;
}

/// Parses a AccountRealloc instruction.
ParsedAccountReallocInstruction parseAccountReallocInstruction(
  Instruction instruction,
) {
  return switch (identifyAccountReallocInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AccountReallocInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    AccountReallocInstruction.realloc => ParsedRealloc(
      data: parseReallocInstruction(instruction),
    ),
    AccountReallocInstruction.realloc2 => ParsedRealloc2(
      data: parseRealloc2Instruction(instruction),
    ),
  };
}
