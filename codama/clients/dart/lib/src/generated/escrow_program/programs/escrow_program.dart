// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the EscrowProgram program.
const escrowProgramProgramAddress = Address(
  '4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT',
);

/// Known accounts for the EscrowProgram program.
enum EscrowProgramAccount { escrowState }

/// Known instructions for the EscrowProgram program.
enum EscrowProgramInstruction { make, take }

/// Identifies the type of a EscrowProgram instruction.
EscrowProgramInstruction identifyEscrowProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return EscrowProgramInstruction.make;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return EscrowProgramInstruction.take;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'escrowProgram',
  });
}

/// A parsed instruction from the EscrowProgram program.
sealed class ParsedEscrowProgramInstruction {
  const ParsedEscrowProgramInstruction(this.instructionType);

  final EscrowProgramInstruction instructionType;
}

/// A parsed Make instruction.
final class ParsedMake extends ParsedEscrowProgramInstruction {
  const ParsedMake({required this.data}) : super(EscrowProgramInstruction.make);

  final MakeInstructionData data;
}

/// A parsed Take instruction.
final class ParsedTake extends ParsedEscrowProgramInstruction {
  const ParsedTake({required this.data}) : super(EscrowProgramInstruction.take);

  final TakeInstructionData data;
}

/// Parses a EscrowProgram instruction.
ParsedEscrowProgramInstruction parseEscrowProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyEscrowProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    EscrowProgramInstruction.make => ParsedMake(
      data: parseMakeInstruction(instruction),
    ),
    EscrowProgramInstruction.take => ParsedTake(
      data: parseTakeInstruction(instruction),
    ),
  };
}
