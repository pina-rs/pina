// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorSystemAccounts program.
const anchorSystemAccountsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the AnchorSystemAccounts program.
enum AnchorSystemAccountsInstruction { initialize }

/// Identifies the type of a AnchorSystemAccounts instruction.
AnchorSystemAccountsInstruction identifyAnchorSystemAccountsInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorSystemAccountsInstruction.initialize;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorSystemAccounts',
  });
}

/// A parsed instruction from the AnchorSystemAccounts program.
sealed class ParsedAnchorSystemAccountsInstruction {
  const ParsedAnchorSystemAccountsInstruction(this.instructionType);

  final AnchorSystemAccountsInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedAnchorSystemAccountsInstruction {
  const ParsedInitialize({required this.data})
    : super(AnchorSystemAccountsInstruction.initialize);

  final InitializeInstructionData data;
}

/// Parses a AnchorSystemAccounts instruction.
ParsedAnchorSystemAccountsInstruction parseAnchorSystemAccountsInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorSystemAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorSystemAccountsInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
  };
}
