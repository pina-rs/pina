// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the Events program.
const eventsProgramAddress = Address(
  '2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy',
);

/// Known instructions for the Events program.
enum EventsInstruction { initialize, testEvent, testEventCpi }

/// Identifies the type of a Events instruction.
EventsInstruction identifyEventsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return EventsInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return EventsInstruction.testEvent;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return EventsInstruction.testEventCpi;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'events',
  });
}

/// A parsed instruction from the Events program.
sealed class ParsedEventsInstruction {
  const ParsedEventsInstruction(this.instructionType);

  final EventsInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedEventsInstruction {
  const ParsedInitialize({required this.data})
    : super(EventsInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed TestEvent instruction.
final class ParsedTestEvent extends ParsedEventsInstruction {
  const ParsedTestEvent({required this.data})
    : super(EventsInstruction.testEvent);

  final TestEventInstructionData data;
}

/// A parsed TestEventCpi instruction.
final class ParsedTestEventCpi extends ParsedEventsInstruction {
  const ParsedTestEventCpi({required this.data})
    : super(EventsInstruction.testEventCpi);

  final TestEventCpiInstructionData data;
}

/// Parses a Events instruction.
ParsedEventsInstruction parseEventsInstruction(Instruction instruction) {
  return switch (identifyEventsInstruction(instruction.data ?? Uint8List(0))) {
    EventsInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    EventsInstruction.testEvent => ParsedTestEvent(
      data: parseTestEventInstruction(instruction),
    ),
    EventsInstruction.testEventCpi => ParsedTestEventCpi(
      data: parseTestEventCpiInstruction(instruction),
    ),
  };
}
