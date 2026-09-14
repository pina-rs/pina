// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the DuplicateMutableAccountsProgram program.
const duplicateMutableAccountsProgramProgramAddress = Address('4D6rvpR7TSPwmFottLGa5gpzMcJ76kN8bimQHV9rogjH');

/// Known instructions for the DuplicateMutableAccountsProgram program.
enum DuplicateMutableAccountsProgramInstruction {
  failsDuplicateMutable,
  allowsDuplicateMutable,
  allowsDuplicateReadonly,
}

/// Identifies the type of a DuplicateMutableAccountsProgram instruction.
DuplicateMutableAccountsProgramInstruction identifyDuplicateMutableAccountsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return DuplicateMutableAccountsProgramInstruction.failsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return DuplicateMutableAccountsProgramInstruction.allowsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return DuplicateMutableAccountsProgramInstruction.allowsDuplicateReadonly;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'duplicateMutableAccountsProgram',
    },
  );
}

/// A parsed instruction from the DuplicateMutableAccountsProgram program.
sealed class ParsedDuplicateMutableAccountsProgramInstruction {
  const ParsedDuplicateMutableAccountsProgramInstruction(this.instructionType);

  final DuplicateMutableAccountsProgramInstruction instructionType;
}

/// A parsed FailsDuplicateMutable instruction.
final class ParsedFailsDuplicateMutable extends ParsedDuplicateMutableAccountsProgramInstruction {
  const ParsedFailsDuplicateMutable({required this.data})
      : super(DuplicateMutableAccountsProgramInstruction.failsDuplicateMutable);

  final FailsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateMutable instruction.
final class ParsedAllowsDuplicateMutable extends ParsedDuplicateMutableAccountsProgramInstruction {
  const ParsedAllowsDuplicateMutable({required this.data})
      : super(DuplicateMutableAccountsProgramInstruction.allowsDuplicateMutable);

  final AllowsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateReadonly instruction.
final class ParsedAllowsDuplicateReadonly extends ParsedDuplicateMutableAccountsProgramInstruction {
  const ParsedAllowsDuplicateReadonly({required this.data})
      : super(DuplicateMutableAccountsProgramInstruction.allowsDuplicateReadonly);

  final AllowsDuplicateReadonlyInstructionData data;
}

/// Parses a DuplicateMutableAccountsProgram instruction.
ParsedDuplicateMutableAccountsProgramInstruction parseDuplicateMutableAccountsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyDuplicateMutableAccountsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    DuplicateMutableAccountsProgramInstruction.failsDuplicateMutable => ParsedFailsDuplicateMutable(
      data: parseFailsDuplicateMutableInstruction(instruction),
    ),
    DuplicateMutableAccountsProgramInstruction.allowsDuplicateMutable => ParsedAllowsDuplicateMutable(
      data: parseAllowsDuplicateMutableInstruction(instruction),
    ),
    DuplicateMutableAccountsProgramInstruction.allowsDuplicateReadonly => ParsedAllowsDuplicateReadonly(
      data: parseAllowsDuplicateReadonlyInstruction(instruction),
    ),
  };
}
