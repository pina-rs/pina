// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the SysvarChecksProgram program.
const sysvarChecksProgramProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the SysvarChecksProgram program.
enum SysvarChecksProgramInstruction { sysvars }

/// Identifies the type of a SysvarChecksProgram instruction.
SysvarChecksProgramInstruction identifySysvarChecksProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return SysvarChecksProgramInstruction.sysvars;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'sysvarChecksProgram',
  });
}

/// A parsed instruction from the SysvarChecksProgram program.
sealed class ParsedSysvarChecksProgramInstruction {
  const ParsedSysvarChecksProgramInstruction(this.instructionType);

  final SysvarChecksProgramInstruction instructionType;
}

/// A parsed Sysvars instruction.
final class ParsedSysvars extends ParsedSysvarChecksProgramInstruction {
  const ParsedSysvars({required this.data})
    : super(SysvarChecksProgramInstruction.sysvars);

  final SysvarsInstructionData data;
}

/// Parses a SysvarChecksProgram instruction.
ParsedSysvarChecksProgramInstruction parseSysvarChecksProgramInstruction(
  Instruction instruction,
) {
  return switch (identifySysvarChecksProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    SysvarChecksProgramInstruction.sysvars => ParsedSysvars(
      data: parseSysvarsInstruction(instruction),
    ),
  };
}
