// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the FloatAccountsProgram program.
const floatAccountsProgramProgramAddress = Address('Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS');

/// Known accounts for the FloatAccountsProgram program.
enum FloatAccountsProgramAccount {
  floatDataAccount,
}

/// Known instructions for the FloatAccountsProgram program.
enum FloatAccountsProgramInstruction {
  create,
  update,
}

/// Identifies the type of a FloatAccountsProgram instruction.
FloatAccountsProgramInstruction identifyFloatAccountsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return FloatAccountsProgramInstruction.create;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return FloatAccountsProgramInstruction.update;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'floatAccountsProgram',
    },
  );
}

/// A parsed instruction from the FloatAccountsProgram program.
sealed class ParsedFloatAccountsProgramInstruction {
  const ParsedFloatAccountsProgramInstruction(this.instructionType);

  final FloatAccountsProgramInstruction instructionType;
}

/// A parsed Create instruction.
final class ParsedCreate extends ParsedFloatAccountsProgramInstruction {
  const ParsedCreate({required this.data})
      : super(FloatAccountsProgramInstruction.create);

  final CreateInstructionData data;
}

/// A parsed Update instruction.
final class ParsedUpdate extends ParsedFloatAccountsProgramInstruction {
  const ParsedUpdate({required this.data})
      : super(FloatAccountsProgramInstruction.update);

  final UpdateInstructionData data;
}

/// Parses a FloatAccountsProgram instruction.
ParsedFloatAccountsProgramInstruction parseFloatAccountsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyFloatAccountsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    FloatAccountsProgramInstruction.create => ParsedCreate(
      data: parseCreateInstruction(instruction),
    ),
    FloatAccountsProgramInstruction.update => ParsedUpdate(
      data: parseUpdateInstruction(instruction),
    ),
  };
}
