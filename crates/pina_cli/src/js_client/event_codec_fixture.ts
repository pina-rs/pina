import {
	getStructDecoder,
	getU16Decoder,
	getU16Encoder,
	getU8Decoder,
} from "@solana/kit";

export type ChangedEvent = {
	discriminator: number;
	migrationVersion: number;
	value: number;
};

export function getChangedEventDiscriminatorBytes() {
	return getU16Encoder().encode(0x1234);
}

export function getChangedEventDecoder() {
	return getStructDecoder([
		["discriminator", getU16Decoder()],
		["migrationVersion", getU8Decoder()],
		["value", getU8Decoder()],
	]);
}
