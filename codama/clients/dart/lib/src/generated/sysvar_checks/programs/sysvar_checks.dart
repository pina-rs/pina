// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the SysvarChecks program.
const sysvarChecksProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the SysvarChecks program.
enum SysvarChecksInstruction { sysvars }

/// Identifies the type of a SysvarChecks instruction.
SysvarChecksInstruction identifySysvarChecksInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return SysvarChecksInstruction.sysvars;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'sysvarChecks',
  });
}

/// A parsed instruction from the SysvarChecks program.
sealed class ParsedSysvarChecksInstruction {
  const ParsedSysvarChecksInstruction(this.instructionType);

  final SysvarChecksInstruction instructionType;
}

/// A parsed Sysvars instruction.
final class ParsedSysvars extends ParsedSysvarChecksInstruction {
  const ParsedSysvars({required this.data})
    : super(SysvarChecksInstruction.sysvars);

  final SysvarsInstructionData data;
}

/// Parses a SysvarChecks instruction.
ParsedSysvarChecksInstruction parseSysvarChecksInstruction(
  Instruction instruction,
) {
  return switch (identifySysvarChecksInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    SysvarChecksInstruction.sysvars => ParsedSysvars(
      data: parseSysvarsInstruction(instruction),
    ),
  };
}
