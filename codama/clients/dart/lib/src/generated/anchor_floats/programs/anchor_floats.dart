// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorFloats program.
const anchorFloatsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known accounts for the AnchorFloats program.
enum AnchorFloatsAccount { floatDataAccount }

/// Known instructions for the AnchorFloats program.
enum AnchorFloatsInstruction { create, update }

/// Identifies the type of a AnchorFloats instruction.
AnchorFloatsInstruction identifyAnchorFloatsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorFloatsInstruction.create;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AnchorFloatsInstruction.update;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorFloats',
  });
}

/// A parsed instruction from the AnchorFloats program.
sealed class ParsedAnchorFloatsInstruction {
  const ParsedAnchorFloatsInstruction(this.instructionType);

  final AnchorFloatsInstruction instructionType;
}

/// A parsed Create instruction.
final class ParsedCreate extends ParsedAnchorFloatsInstruction {
  const ParsedCreate({required this.data})
    : super(AnchorFloatsInstruction.create);

  final CreateInstructionData data;
}

/// A parsed Update instruction.
final class ParsedUpdate extends ParsedAnchorFloatsInstruction {
  const ParsedUpdate({required this.data})
    : super(AnchorFloatsInstruction.update);

  final UpdateInstructionData data;
}

/// Parses a AnchorFloats instruction.
ParsedAnchorFloatsInstruction parseAnchorFloatsInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorFloatsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorFloatsInstruction.create => ParsedCreate(
      data: parseCreateInstruction(instruction),
    ),
    AnchorFloatsInstruction.update => ParsedUpdate(
      data: parseUpdateInstruction(instruction),
    ),
  };
}
