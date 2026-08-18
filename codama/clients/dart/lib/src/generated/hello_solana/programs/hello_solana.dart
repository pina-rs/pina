// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the HelloSolana program.
const helloSolanaProgramAddress = Address(
  'DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A',
);

/// Known instructions for the HelloSolana program.
enum HelloSolanaInstruction { hello }

/// Identifies the type of a HelloSolana instruction.
HelloSolanaInstruction identifyHelloSolanaInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return HelloSolanaInstruction.hello;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'helloSolana',
  });
}

/// A parsed instruction from the HelloSolana program.
sealed class ParsedHelloSolanaInstruction {
  const ParsedHelloSolanaInstruction(this.instructionType);

  final HelloSolanaInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedHelloSolanaInstruction {
  const ParsedHello({required this.data}) : super(HelloSolanaInstruction.hello);

  final HelloInstructionData data;
}

/// Parses a HelloSolana instruction.
ParsedHelloSolanaInstruction parseHelloSolanaInstruction(
  Instruction instruction,
) {
  return switch (identifyHelloSolanaInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    HelloSolanaInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
  };
}
