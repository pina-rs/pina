// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the MigrationsProgram program.
const migrationsProgramProgramAddress = Address(
  'GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS',
);

/// Known accounts for the MigrationsProgram program.
enum MigrationsProgramAccount { state, manualState, compactState }

/// Known instructions for the MigrationsProgram program.
enum MigrationsProgramInstruction { update, relay }

/// Identifies the type of a MigrationsProgram instruction.
MigrationsProgramInstruction identifyMigrationsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0) &&
      containsBytes(data, getU8Encoder().encode(2), 1)) {
    return MigrationsProgramInstruction.update;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return MigrationsProgramInstruction.relay;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'migrationsProgram',
  });
}

/// A parsed instruction from the MigrationsProgram program.
sealed class ParsedMigrationsProgramInstruction {
  const ParsedMigrationsProgramInstruction(this.instructionType);

  final MigrationsProgramInstruction instructionType;
}

/// A parsed Update instruction.
final class ParsedUpdate extends ParsedMigrationsProgramInstruction {
  const ParsedUpdate({required this.data})
    : super(MigrationsProgramInstruction.update);

  final UpdateInstructionData data;
}

/// A parsed Relay instruction.
final class ParsedRelay extends ParsedMigrationsProgramInstruction {
  const ParsedRelay({required this.data})
    : super(MigrationsProgramInstruction.relay);

  final RelayInstructionData data;
}

/// Parses a MigrationsProgram instruction.
ParsedMigrationsProgramInstruction parseMigrationsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyMigrationsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    MigrationsProgramInstruction.update => ParsedUpdate(
      data: parseUpdateInstruction(instruction),
    ),
    MigrationsProgramInstruction.relay => ParsedRelay(
      data: parseRelayInstruction(instruction),
    ),
  };
}
