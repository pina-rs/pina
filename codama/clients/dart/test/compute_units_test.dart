import 'dart:typed_data';

import 'package:pina_codama_clients/counter_program.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';
import 'package:test/test.dart';

const systemAddress = Address('11111111111111111111111111111111');
const forkAddress = Address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');

Instruction counterInstruction(List<int> data, {Address? programAddress}) {
  return Instruction(
    programAddress: programAddress ?? counterProgramProgramAddress,
    data: Uint8List.fromList(data),
  );
}

void main() {
  group('getCounterProgramComputeUnitLimit', () {
    final initialize = counterInstruction([0, 254]);
    final increment = counterInstruction([1]);

    test('each recorded limit covers its measurement', () {
      expect(
        incrementComputeUnitLimit,
        greaterThan(incrementMeasuredComputeUnits),
      );
      expect(
        initializeComputeUnitLimit,
        greaterThan(initializeMeasuredComputeUnits),
      );
    });

    test("sums the recorded limits of this program's instructions", () {
      expect(
        getCounterProgramComputeUnitLimit([increment]),
        incrementComputeUnitLimit,
      );
      expect(
        getCounterProgramComputeUnitLimit([
          initialize,
          Instruction(
            programAddress: systemAddress,
            data: Uint8List.fromList([2, 0, 0, 0]),
          ),
          increment,
        ]),
        initializeComputeUnitLimit + incrementComputeUnitLimit,
      );
    });

    test('returns null without a measured instruction for this program', () {
      expect(getCounterProgramComputeUnitLimit([]), isNull);
      expect(
        getCounterProgramComputeUnitLimit([
          const Instruction(programAddress: systemAddress),
        ]),
        isNull,
      );
      expect(
        getCounterProgramComputeUnitLimit([
          increment,
          counterInstruction([9]),
        ]),
        isNull,
      );
      expect(
        getCounterProgramComputeUnitLimit([
          const Instruction(programAddress: counterProgramProgramAddress),
        ]),
        isNull,
      );
      expect(
        getCounterProgramComputeUnitLimit([counterInstruction([])]),
        isNull,
      );
    });

    test('counts a program deployed at another address when told to', () {
      final forked = counterInstruction([1], programAddress: forkAddress);

      expect(getCounterProgramComputeUnitLimit([forked]), isNull);
      expect(
        getCounterProgramComputeUnitLimit([
          forked,
        ], programAddress: forkAddress),
        incrementComputeUnitLimit,
      );
    });

    test('caps the sum at the transaction maximum', () {
      expect(
        getCounterProgramComputeUnitLimit(List.filled(10000, increment)),
        1400000,
      );
    });
  });
}
