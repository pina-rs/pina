// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the PropAmmProgram program.
const propAmmProgramProgramAddress = Address(
  '55555555555555555555555555555555555555555555',
);

/// Known accounts for the PropAmmProgram program.
enum PropAmmProgramAccount { oracleState }

/// Known instructions for the PropAmmProgram program.
enum PropAmmProgramInstruction { initialize, update, rotateAuthority }

/// Identifies the type of a PropAmmProgram instruction.
PropAmmProgramInstruction identifyPropAmmProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return PropAmmProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return PropAmmProgramInstruction.update;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return PropAmmProgramInstruction.rotateAuthority;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'propAmmProgram',
  });
}

/// A parsed instruction from the PropAmmProgram program.
sealed class ParsedPropAmmProgramInstruction {
  const ParsedPropAmmProgramInstruction(this.instructionType);

  final PropAmmProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedPropAmmProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(PropAmmProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Update instruction.
final class ParsedUpdate extends ParsedPropAmmProgramInstruction {
  const ParsedUpdate({required this.data})
    : super(PropAmmProgramInstruction.update);

  final UpdateInstructionData data;
}

/// A parsed RotateAuthority instruction.
final class ParsedRotateAuthority extends ParsedPropAmmProgramInstruction {
  const ParsedRotateAuthority({required this.data})
    : super(PropAmmProgramInstruction.rotateAuthority);

  final RotateAuthorityInstructionData data;
}

/// Parses a PropAmmProgram instruction.
ParsedPropAmmProgramInstruction parsePropAmmProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyPropAmmProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    PropAmmProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    PropAmmProgramInstruction.update => ParsedUpdate(
      data: parseUpdateInstruction(instruction),
    ),
    PropAmmProgramInstruction.rotateAuthority => ParsedRotateAuthority(
      data: parseRotateAuthorityInstruction(instruction),
    ),
  };
}
