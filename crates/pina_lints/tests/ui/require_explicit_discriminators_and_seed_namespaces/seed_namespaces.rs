#![allow(dead_code)]

struct AccountView;

impl AccountView {
	fn assert_seeds(&self, _seeds: &[&[u8]]) {}
}

struct State;

impl State {
	fn seeds(_authority: &[u8]) -> [&'static [u8]; 1] {
		[b"state"]
	}
}

const SEED_CONFIG: &[u8] = b"config";
const VAULT_SEED: &[u8] = b"vault";
const SEED: &[u8] = b"generic";

fn assert_seeds(_seeds: &[&[u8]]) {}

fn process_named_constant(account: &AccountView, authority: &[u8]) {
	let seeds = &[SEED_CONFIG, authority];
	account.assert_seeds(seeds);
}

fn process_suffix_constant(account: &AccountView, authority: &[u8]) {
	let seeds = &[VAULT_SEED, authority];
	account.assert_seeds(seeds);
}

fn process_generated_builder(account: &AccountView, authority: &[u8]) {
	let seeds = State::seeds(authority);
	account.assert_seeds(&seeds);
}

fn process_named_seed(account: &AccountView, authority: &[u8]) {
	let seeds = &[SEED, authority];
	account.assert_seeds(seeds);
}

fn process_inline_namespace(account: &AccountView, authority: &[u8]) {
	let seeds = &[b"inline".as_slice(), authority];
	account.assert_seeds(seeds);
}

fn process_missing_namespace(account: &AccountView, authority: &[u8]) {
	let seeds = &[authority];
	account.assert_seeds(seeds);
	//~^ WARN: seed-based example code should use explicit byte-string namespaces and visible discriminator markers
}

fn process_local_helper(authority: &[u8]) {
	let seeds = &[authority];
	assert_seeds(seeds);
	//~^ WARN: seed-based example code should use explicit byte-string namespaces and visible discriminator markers
}

fn main() {}
