// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the SystemAccounts program.
const systemAccountsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the SystemAccounts program.
enum SystemAccountsInstruction { initialize }

/// Identifies the type of a SystemAccounts instruction.
SystemAccountsInstruction identifySystemAccountsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return SystemAccountsInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'systemAccounts',
  });
}

/// A parsed instruction from the SystemAccounts program.
sealed class ParsedSystemAccountsInstruction {
  const ParsedSystemAccountsInstruction(this.instructionType);

  final SystemAccountsInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedSystemAccountsInstruction {
  const ParsedInitialize({required this.data})
    : super(SystemAccountsInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a SystemAccounts instruction.
ParsedSystemAccountsInstruction parseSystemAccountsInstruction(
  Instruction instruction,
) {
  return switch (identifySystemAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    SystemAccountsInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
