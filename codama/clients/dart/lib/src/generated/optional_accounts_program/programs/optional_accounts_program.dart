// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the OptionalAccountsProgram program.
const optionalAccountsProgramProgramAddress = Address(
  'ccdMMVpwebk8NxwJdY4CndxkLKUTM6fkaFUteAfFeci',
);

/// Known accounts for the OptionalAccountsProgram program.
enum OptionalAccountsProgramAccount { storeState }

/// Known instructions for the OptionalAccountsProgram program.
enum OptionalAccountsProgramInstruction { init, touch, inspect, note }

/// Identifies the type of a OptionalAccountsProgram instruction.
OptionalAccountsProgramInstruction identifyOptionalAccountsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return OptionalAccountsProgramInstruction.init;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return OptionalAccountsProgramInstruction.touch;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return OptionalAccountsProgramInstruction.inspect;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return OptionalAccountsProgramInstruction.note;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'optionalAccountsProgram',
  });
}

/// A parsed instruction from the OptionalAccountsProgram program.
sealed class ParsedOptionalAccountsProgramInstruction {
  const ParsedOptionalAccountsProgramInstruction(this.instructionType);

  final OptionalAccountsProgramInstruction instructionType;
}

/// A parsed Init instruction.
final class ParsedInit extends ParsedOptionalAccountsProgramInstruction {
  const ParsedInit({required this.data})
    : super(OptionalAccountsProgramInstruction.init);

  final InitInstructionData data;
}

/// A parsed Touch instruction.
final class ParsedTouch extends ParsedOptionalAccountsProgramInstruction {
  const ParsedTouch({required this.data})
    : super(OptionalAccountsProgramInstruction.touch);

  final TouchInstructionData data;
}

/// A parsed Inspect instruction.
final class ParsedInspect extends ParsedOptionalAccountsProgramInstruction {
  const ParsedInspect({required this.data})
    : super(OptionalAccountsProgramInstruction.inspect);

  final InspectInstructionData data;
}

/// A parsed Note instruction.
final class ParsedNote extends ParsedOptionalAccountsProgramInstruction {
  const ParsedNote({required this.data})
    : super(OptionalAccountsProgramInstruction.note);

  final NoteInstructionData data;
}

/// Parses a OptionalAccountsProgram instruction.
ParsedOptionalAccountsProgramInstruction
parseOptionalAccountsProgramInstruction(Instruction instruction) {
  return switch (identifyOptionalAccountsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    OptionalAccountsProgramInstruction.init => ParsedInit(
      data: parseInitInstruction(instruction),
    ),
    OptionalAccountsProgramInstruction.touch => ParsedTouch(
      data: parseTouchInstruction(instruction),
    ),
    OptionalAccountsProgramInstruction.inspect => ParsedInspect(
      data: parseInspectInstruction(instruction),
    ),
    OptionalAccountsProgramInstruction.note => ParsedNote(
      data: parseNoteInstruction(instruction),
    ),
  };
}
