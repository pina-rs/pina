// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the EventsProgram program.
const eventsProgramProgramAddress = Address(
  '2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy',
);

/// Known instructions for the EventsProgram program.
enum EventsProgramInstruction { initialize, testEvent, testEventCpi }

/// Identifies the type of a EventsProgram instruction.
EventsProgramInstruction identifyEventsProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return EventsProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return EventsProgramInstruction.testEvent;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return EventsProgramInstruction.testEventCpi;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'eventsProgram',
  });
}

/// A parsed instruction from the EventsProgram program.
sealed class ParsedEventsProgramInstruction {
  const ParsedEventsProgramInstruction(this.instructionType);

  final EventsProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedEventsProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(EventsProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed TestEvent instruction.
final class ParsedTestEvent extends ParsedEventsProgramInstruction {
  const ParsedTestEvent({required this.data})
    : super(EventsProgramInstruction.testEvent);

  final TestEventInstructionData data;
}

/// A parsed TestEventCpi instruction.
final class ParsedTestEventCpi extends ParsedEventsProgramInstruction {
  const ParsedTestEventCpi({required this.data})
    : super(EventsProgramInstruction.testEventCpi);

  final TestEventCpiInstructionData data;
}

/// Parses a EventsProgram instruction.
ParsedEventsProgramInstruction parseEventsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyEventsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    EventsProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    EventsProgramInstruction.testEvent => ParsedTestEvent(
      data: parseTestEventInstruction(instruction),
    ),
    EventsProgramInstruction.testEventCpi => ParsedTestEventCpi(
      data: parseTestEventCpiInstruction(instruction),
    ),
  };
}
