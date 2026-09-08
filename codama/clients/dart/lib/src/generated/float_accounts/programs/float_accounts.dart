// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the FloatAccounts program.
const floatAccountsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known accounts for the FloatAccounts program.
enum FloatAccountsAccount { floatDataAccount }

/// Known instructions for the FloatAccounts program.
enum FloatAccountsInstruction { create, update }

/// Identifies the type of a FloatAccounts instruction.
FloatAccountsInstruction identifyFloatAccountsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return FloatAccountsInstruction.create;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return FloatAccountsInstruction.update;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'floatAccounts',
  });
}

/// A parsed instruction from the FloatAccounts program.
sealed class ParsedFloatAccountsInstruction {
  const ParsedFloatAccountsInstruction(this.instructionType);

  final FloatAccountsInstruction instructionType;
}

/// A parsed Create instruction.
final class ParsedCreate extends ParsedFloatAccountsInstruction {
  const ParsedCreate({required this.data})
    : super(FloatAccountsInstruction.create);

  final CreateInstructionData data;
}

/// A parsed Update instruction.
final class ParsedUpdate extends ParsedFloatAccountsInstruction {
  const ParsedUpdate({required this.data})
    : super(FloatAccountsInstruction.update);

  final UpdateInstructionData data;
}

/// Parses a FloatAccounts instruction.
ParsedFloatAccountsInstruction parseFloatAccountsInstruction(
  Instruction instruction,
) {
  return switch (identifyFloatAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    FloatAccountsInstruction.create => ParsedCreate(
      data: parseCreateInstruction(instruction),
    ),
    FloatAccountsInstruction.update => ParsedUpdate(
      data: parseUpdateInstruction(instruction),
    ),
  };
}
