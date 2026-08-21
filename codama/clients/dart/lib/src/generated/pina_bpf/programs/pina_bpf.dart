// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the PinaBpf program.
const pinaBpfProgramAddress = Address(
  '2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe',
);

/// Known accounts for the PinaBpf program.
enum PinaBpfAccount { state }

/// Known instructions for the PinaBpf program.
enum PinaBpfInstruction {
  hello,
  forwardRotateWithSigner,
  forwardRotateWithPda,
  createPda,
}

/// Identifies the type of a PinaBpf instruction.
PinaBpfInstruction identifyPinaBpfInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return PinaBpfInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return PinaBpfInstruction.forwardRotateWithSigner;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return PinaBpfInstruction.forwardRotateWithPda;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return PinaBpfInstruction.createPda;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'pinaBpf',
  });
}

/// A parsed instruction from the PinaBpf program.
sealed class ParsedPinaBpfInstruction {
  const ParsedPinaBpfInstruction(this.instructionType);

  final PinaBpfInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedPinaBpfInstruction {
  const ParsedHello({required this.data}) : super(PinaBpfInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed ForwardRotateWithSigner instruction.
final class ParsedForwardRotateWithSigner extends ParsedPinaBpfInstruction {
  const ParsedForwardRotateWithSigner({required this.data})
    : super(PinaBpfInstruction.forwardRotateWithSigner);

  final ForwardRotateWithSignerInstructionData data;
}

/// A parsed ForwardRotateWithPda instruction.
final class ParsedForwardRotateWithPda extends ParsedPinaBpfInstruction {
  const ParsedForwardRotateWithPda({required this.data})
    : super(PinaBpfInstruction.forwardRotateWithPda);

  final ForwardRotateWithPdaInstructionData data;
}

/// A parsed CreatePda instruction.
final class ParsedCreatePda extends ParsedPinaBpfInstruction {
  const ParsedCreatePda({required this.data})
    : super(PinaBpfInstruction.createPda);

  final CreatePdaInstructionData data;
}

/// Parses a PinaBpf instruction.
ParsedPinaBpfInstruction parsePinaBpfInstruction(Instruction instruction) {
  return switch (identifyPinaBpfInstruction(instruction.data ?? Uint8List(0))) {
    PinaBpfInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    PinaBpfInstruction.forwardRotateWithSigner => ParsedForwardRotateWithSigner(
      data: parseForwardRotateWithSignerInstruction(instruction),
    ),
    PinaBpfInstruction.forwardRotateWithPda => ParsedForwardRotateWithPda(
      data: parseForwardRotateWithPdaInstruction(instruction),
    ),
    PinaBpfInstruction.createPda => ParsedCreatePda(
      data: parseCreatePdaInstruction(instruction),
    ),
  };
}
