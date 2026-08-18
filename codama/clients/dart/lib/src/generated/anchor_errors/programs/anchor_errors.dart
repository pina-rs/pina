// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorErrors program.
const anchorErrorsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the AnchorErrors program.
enum AnchorErrorsInstruction {
  hello,
  helloNoMsg,
  helloNext,
  requireEq,
  requireNeq,
  requireGt,
  requireGte,
}

/// Identifies the type of a AnchorErrors instruction.
AnchorErrorsInstruction identifyAnchorErrorsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorErrorsInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AnchorErrorsInstruction.helloNoMsg;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return AnchorErrorsInstruction.helloNext;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return AnchorErrorsInstruction.requireEq;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0)) {
    return AnchorErrorsInstruction.requireNeq;
  }
  if (containsBytes(data, getU8Encoder().encode(5), 0)) {
    return AnchorErrorsInstruction.requireGt;
  }
  if (containsBytes(data, getU8Encoder().encode(6), 0)) {
    return AnchorErrorsInstruction.requireGte;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorErrors',
  });
}

/// A parsed instruction from the AnchorErrors program.
sealed class ParsedAnchorErrorsInstruction {
  const ParsedAnchorErrorsInstruction(this.instructionType);

  final AnchorErrorsInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedAnchorErrorsInstruction {
  const ParsedHello({required this.data})
    : super(AnchorErrorsInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed HelloNoMsg instruction.
final class ParsedHelloNoMsg extends ParsedAnchorErrorsInstruction {
  const ParsedHelloNoMsg({required this.data})
    : super(AnchorErrorsInstruction.helloNoMsg);

  final HelloNoMsgInstructionData data;
}

/// A parsed HelloNext instruction.
final class ParsedHelloNext extends ParsedAnchorErrorsInstruction {
  const ParsedHelloNext({required this.data})
    : super(AnchorErrorsInstruction.helloNext);

  final HelloNextInstructionData data;
}

/// A parsed RequireEq instruction.
final class ParsedRequireEq extends ParsedAnchorErrorsInstruction {
  const ParsedRequireEq({required this.data})
    : super(AnchorErrorsInstruction.requireEq);

  final RequireEqInstructionData data;
}

/// A parsed RequireNeq instruction.
final class ParsedRequireNeq extends ParsedAnchorErrorsInstruction {
  const ParsedRequireNeq({required this.data})
    : super(AnchorErrorsInstruction.requireNeq);

  final RequireNeqInstructionData data;
}

/// A parsed RequireGt instruction.
final class ParsedRequireGt extends ParsedAnchorErrorsInstruction {
  const ParsedRequireGt({required this.data})
    : super(AnchorErrorsInstruction.requireGt);

  final RequireGtInstructionData data;
}

/// A parsed RequireGte instruction.
final class ParsedRequireGte extends ParsedAnchorErrorsInstruction {
  const ParsedRequireGte({required this.data})
    : super(AnchorErrorsInstruction.requireGte);

  final RequireGteInstructionData data;
}

/// Parses a AnchorErrors instruction.
ParsedAnchorErrorsInstruction parseAnchorErrorsInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorErrorsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorErrorsInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    AnchorErrorsInstruction.helloNoMsg => ParsedHelloNoMsg(
      data: parseHelloNoMsgInstruction(instruction),
    ),
    AnchorErrorsInstruction.helloNext => ParsedHelloNext(
      data: parseHelloNextInstruction(instruction),
    ),
    AnchorErrorsInstruction.requireEq => ParsedRequireEq(
      data: parseRequireEqInstruction(instruction),
    ),
    AnchorErrorsInstruction.requireNeq => ParsedRequireNeq(
      data: parseRequireNeqInstruction(instruction),
    ),
    AnchorErrorsInstruction.requireGt => ParsedRequireGt(
      data: parseRequireGtInstruction(instruction),
    ),
    AnchorErrorsInstruction.requireGte => ParsedRequireGte(
      data: parseRequireGteInstruction(instruction),
    ),
  };
}
