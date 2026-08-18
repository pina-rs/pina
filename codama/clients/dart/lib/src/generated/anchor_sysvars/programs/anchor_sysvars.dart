// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorSysvars program.
const anchorSysvarsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the AnchorSysvars program.
enum AnchorSysvarsInstruction { sysvars }

/// Identifies the type of a AnchorSysvars instruction.
AnchorSysvarsInstruction identifyAnchorSysvarsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorSysvarsInstruction.sysvars;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorSysvars',
  });
}

/// A parsed instruction from the AnchorSysvars program.
sealed class ParsedAnchorSysvarsInstruction {
  const ParsedAnchorSysvarsInstruction(this.instructionType);

  final AnchorSysvarsInstruction instructionType;
}

/// A parsed Sysvars instruction.
final class ParsedSysvars extends ParsedAnchorSysvarsInstruction {
  const ParsedSysvars({required this.data})
    : super(AnchorSysvarsInstruction.sysvars);

  final SysvarsInstructionData data;
}

/// Parses a AnchorSysvars instruction.
ParsedAnchorSysvarsInstruction parseAnchorSysvarsInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorSysvarsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorSysvarsInstruction.sysvars => ParsedSysvars(
      data: parseSysvarsInstruction(instruction),
    ),
  };
}
