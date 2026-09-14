// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the ProfileProgram program.
const profileProgramProgramAddress = Address(
  '6oW4PDgWpZGWqAEZNvqnAtQi8GotATsxxjCLYQpZJhHL',
);

/// Known accounts for the ProfileProgram program.
enum ProfileProgramAccount { profileState }

/// Known instructions for the ProfileProgram program.
enum ProfileProgramInstruction { initialize, updateProfile, addTag, removeTag }

/// Identifies the type of a ProfileProgram instruction.
ProfileProgramInstruction identifyProfileProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return ProfileProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return ProfileProgramInstruction.updateProfile;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return ProfileProgramInstruction.addTag;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return ProfileProgramInstruction.removeTag;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'profileProgram',
  });
}

/// A parsed instruction from the ProfileProgram program.
sealed class ParsedProfileProgramInstruction {
  const ParsedProfileProgramInstruction(this.instructionType);

  final ProfileProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedProfileProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(ProfileProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed UpdateProfile instruction.
final class ParsedUpdateProfile extends ParsedProfileProgramInstruction {
  const ParsedUpdateProfile({required this.data})
    : super(ProfileProgramInstruction.updateProfile);

  final UpdateProfileInstructionData data;
}

/// A parsed AddTag instruction.
final class ParsedAddTag extends ParsedProfileProgramInstruction {
  const ParsedAddTag({required this.data})
    : super(ProfileProgramInstruction.addTag);

  final AddTagInstructionData data;
}

/// A parsed RemoveTag instruction.
final class ParsedRemoveTag extends ParsedProfileProgramInstruction {
  const ParsedRemoveTag({required this.data})
    : super(ProfileProgramInstruction.removeTag);

  final RemoveTagInstructionData data;
}

/// Parses a ProfileProgram instruction.
ParsedProfileProgramInstruction parseProfileProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyProfileProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    ProfileProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    ProfileProgramInstruction.updateProfile => ParsedUpdateProfile(
      data: parseUpdateProfileInstruction(instruction),
    ),
    ProfileProgramInstruction.addTag => ParsedAddTag(
      data: parseAddTagInstruction(instruction),
    ),
    ProfileProgramInstruction.removeTag => ParsedRemoveTag(
      data: parseRemoveTagInstruction(instruction),
    ),
  };
}
