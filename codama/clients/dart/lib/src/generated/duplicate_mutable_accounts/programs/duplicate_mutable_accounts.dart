// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the DuplicateMutableAccounts program.
const duplicateMutableAccountsProgramAddress = Address(
  '4D6rvpR7TSPwmFottLGa5gpzMcJ76kN8bimQHV9rogjH',
);

/// Known instructions for the DuplicateMutableAccounts program.
enum DuplicateMutableAccountsInstruction {
  failsDuplicateMutable,
  allowsDuplicateMutable,
  allowsDuplicateReadonly,
}

/// Identifies the type of a DuplicateMutableAccounts instruction.
DuplicateMutableAccountsInstruction identifyDuplicateMutableAccountsInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return DuplicateMutableAccountsInstruction.failsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return DuplicateMutableAccountsInstruction.allowsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return DuplicateMutableAccountsInstruction.allowsDuplicateReadonly;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'duplicateMutableAccounts',
  });
}

/// A parsed instruction from the DuplicateMutableAccounts program.
sealed class ParsedDuplicateMutableAccountsInstruction {
  const ParsedDuplicateMutableAccountsInstruction(this.instructionType);

  final DuplicateMutableAccountsInstruction instructionType;
}

/// A parsed FailsDuplicateMutable instruction.
final class ParsedFailsDuplicateMutable
    extends ParsedDuplicateMutableAccountsInstruction {
  const ParsedFailsDuplicateMutable({required this.data})
    : super(DuplicateMutableAccountsInstruction.failsDuplicateMutable);

  final FailsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateMutable instruction.
final class ParsedAllowsDuplicateMutable
    extends ParsedDuplicateMutableAccountsInstruction {
  const ParsedAllowsDuplicateMutable({required this.data})
    : super(DuplicateMutableAccountsInstruction.allowsDuplicateMutable);

  final AllowsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateReadonly instruction.
final class ParsedAllowsDuplicateReadonly
    extends ParsedDuplicateMutableAccountsInstruction {
  const ParsedAllowsDuplicateReadonly({required this.data})
    : super(DuplicateMutableAccountsInstruction.allowsDuplicateReadonly);

  final AllowsDuplicateReadonlyInstructionData data;
}

/// Parses a DuplicateMutableAccounts instruction.
ParsedDuplicateMutableAccountsInstruction
parseDuplicateMutableAccountsInstruction(Instruction instruction) {
  return switch (identifyDuplicateMutableAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    DuplicateMutableAccountsInstruction.failsDuplicateMutable =>
      ParsedFailsDuplicateMutable(
        data: parseFailsDuplicateMutableInstruction(instruction),
      ),
    DuplicateMutableAccountsInstruction.allowsDuplicateMutable =>
      ParsedAllowsDuplicateMutable(
        data: parseAllowsDuplicateMutableInstruction(instruction),
      ),
    DuplicateMutableAccountsInstruction.allowsDuplicateReadonly =>
      ParsedAllowsDuplicateReadonly(
        data: parseAllowsDuplicateReadonlyInstruction(instruction),
      ),
  };
}
