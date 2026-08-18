// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorEvents program.
const anchorEventsProgramAddress = Address(
  '2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy',
);

/// Known instructions for the AnchorEvents program.
enum AnchorEventsInstruction { initialize, testEvent, testEventCpi }

/// Identifies the type of a AnchorEvents instruction.
AnchorEventsInstruction identifyAnchorEventsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorEventsInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AnchorEventsInstruction.testEvent;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return AnchorEventsInstruction.testEventCpi;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorEvents',
  });
}

/// A parsed instruction from the AnchorEvents program.
sealed class ParsedAnchorEventsInstruction {
  const ParsedAnchorEventsInstruction(this.instructionType);

  final AnchorEventsInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedAnchorEventsInstruction {
  const ParsedInitialize({required this.data})
    : super(AnchorEventsInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed TestEvent instruction.
final class ParsedTestEvent extends ParsedAnchorEventsInstruction {
  const ParsedTestEvent({required this.data})
    : super(AnchorEventsInstruction.testEvent);

  final TestEventInstructionData data;
}

/// A parsed TestEventCpi instruction.
final class ParsedTestEventCpi extends ParsedAnchorEventsInstruction {
  const ParsedTestEventCpi({required this.data})
    : super(AnchorEventsInstruction.testEventCpi);

  final TestEventCpiInstructionData data;
}

/// Parses a AnchorEvents instruction.
ParsedAnchorEventsInstruction parseAnchorEventsInstruction(
  Instruction instruction,
) {
  return switch (identifyAnchorEventsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorEventsInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    AnchorEventsInstruction.testEvent => ParsedTestEvent(
      data: parseTestEventInstruction(instruction),
    ),
    AnchorEventsInstruction.testEventCpi => ParsedTestEventCpi(
      data: parseTestEventCpiInstruction(instruction),
    ),
  };
}
