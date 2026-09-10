// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the PinaBpfProgram program.
const pinaBpfProgramProgramAddress = Address('2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe');

/// Known accounts for the PinaBpfProgram program.
enum PinaBpfProgramAccount {
  state,
}

/// Known instructions for the PinaBpfProgram program.
enum PinaBpfProgramInstruction {
  hello,
  forwardRotateWithSigner,
  forwardRotateWithPda,
  createPda,
}

/// Identifies the type of a PinaBpfProgram instruction.
PinaBpfProgramInstruction identifyPinaBpfProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return PinaBpfProgramInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return PinaBpfProgramInstruction.forwardRotateWithSigner;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return PinaBpfProgramInstruction.forwardRotateWithPda;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return PinaBpfProgramInstruction.createPda;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'pinaBpfProgram',
    },
  );
}

/// A parsed instruction from the PinaBpfProgram program.
sealed class ParsedPinaBpfProgramInstruction {
  const ParsedPinaBpfProgramInstruction(this.instructionType);

  final PinaBpfProgramInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedPinaBpfProgramInstruction {
  const ParsedHello({required this.data})
      : super(PinaBpfProgramInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed ForwardRotateWithSigner instruction.
final class ParsedForwardRotateWithSigner extends ParsedPinaBpfProgramInstruction {
  const ParsedForwardRotateWithSigner({required this.data})
      : super(PinaBpfProgramInstruction.forwardRotateWithSigner);

  final ForwardRotateWithSignerInstructionData data;
}

/// A parsed ForwardRotateWithPda instruction.
final class ParsedForwardRotateWithPda extends ParsedPinaBpfProgramInstruction {
  const ParsedForwardRotateWithPda({required this.data})
      : super(PinaBpfProgramInstruction.forwardRotateWithPda);

  final ForwardRotateWithPdaInstructionData data;
}

/// A parsed CreatePda instruction.
final class ParsedCreatePda extends ParsedPinaBpfProgramInstruction {
  const ParsedCreatePda({required this.data})
      : super(PinaBpfProgramInstruction.createPda);

  final CreatePdaInstructionData data;
}

/// Parses a PinaBpfProgram instruction.
ParsedPinaBpfProgramInstruction parsePinaBpfProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyPinaBpfProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    PinaBpfProgramInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    PinaBpfProgramInstruction.forwardRotateWithSigner => ParsedForwardRotateWithSigner(
      data: parseForwardRotateWithSignerInstruction(instruction),
    ),
    PinaBpfProgramInstruction.forwardRotateWithPda => ParsedForwardRotateWithPda(
      data: parseForwardRotateWithPdaInstruction(instruction),
    ),
    PinaBpfProgramInstruction.createPda => ParsedCreatePda(
      data: parseCreatePdaInstruction(instruction),
    ),
  };
}
