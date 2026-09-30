// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the RoleRegistryProgram program.
const roleRegistryProgramProgramAddress = Address(
  '3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX',
);

/// Known accounts for the RoleRegistryProgram program.
enum RoleRegistryProgramAccount { registryConfig, roleEntry }

/// Known instructions for the RoleRegistryProgram program.
enum RoleRegistryProgramInstruction {
  initialize,
  addRole,
  updateRole,
  deactivateRole,
  rotateAdmin,
}

/// Identifies the type of a RoleRegistryProgram instruction.
RoleRegistryProgramInstruction identifyRoleRegistryProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return RoleRegistryProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return RoleRegistryProgramInstruction.addRole;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return RoleRegistryProgramInstruction.updateRole;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return RoleRegistryProgramInstruction.deactivateRole;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0)) {
    return RoleRegistryProgramInstruction.rotateAdmin;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'roleRegistryProgram',
  });
}

/// A parsed instruction from the RoleRegistryProgram program.
sealed class ParsedRoleRegistryProgramInstruction {
  const ParsedRoleRegistryProgramInstruction(this.instructionType);

  final RoleRegistryProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedRoleRegistryProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(RoleRegistryProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed AddRole instruction.
final class ParsedAddRole extends ParsedRoleRegistryProgramInstruction {
  const ParsedAddRole({required this.data})
    : super(RoleRegistryProgramInstruction.addRole);

  final AddRoleInstructionData data;
}

/// A parsed UpdateRole instruction.
final class ParsedUpdateRole extends ParsedRoleRegistryProgramInstruction {
  const ParsedUpdateRole({required this.data})
    : super(RoleRegistryProgramInstruction.updateRole);

  final UpdateRoleInstructionData data;
}

/// A parsed DeactivateRole instruction.
final class ParsedDeactivateRole extends ParsedRoleRegistryProgramInstruction {
  const ParsedDeactivateRole({required this.data})
    : super(RoleRegistryProgramInstruction.deactivateRole);

  final DeactivateRoleInstructionData data;
}

/// A parsed RotateAdmin instruction.
final class ParsedRotateAdmin extends ParsedRoleRegistryProgramInstruction {
  const ParsedRotateAdmin({required this.data})
    : super(RoleRegistryProgramInstruction.rotateAdmin);

  final RotateAdminInstructionData data;
}

/// Parses a RoleRegistryProgram instruction.
ParsedRoleRegistryProgramInstruction parseRoleRegistryProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyRoleRegistryProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    RoleRegistryProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    RoleRegistryProgramInstruction.addRole => ParsedAddRole(
      data: parseAddRoleInstruction(instruction),
    ),
    RoleRegistryProgramInstruction.updateRole => ParsedUpdateRole(
      data: parseUpdateRoleInstruction(instruction),
    ),
    RoleRegistryProgramInstruction.deactivateRole => ParsedDeactivateRole(
      data: parseDeactivateRoleInstruction(instruction),
    ),
    RoleRegistryProgramInstruction.rotateAdmin => ParsedRotateAdmin(
      data: parseRotateAdminInstruction(instruction),
    ),
  };
}
