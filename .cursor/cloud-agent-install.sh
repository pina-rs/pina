#!/usr/bin/env bash
# Bootstrap the Pina toolchain on a Linux Cloud Agent VM.
#
# The repository's development shell is Nix via devenv (see devenv.nix and
# .github/actions/devenv). Cloud Agent VMs run under tini rather than systemd,
# so this uses Nix's single-user installer instead of the multi-user daemon.
# macOS host-linker overrides and the Windows CLI job are outside this image.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

retry() {
	local attempts="$1"
	shift
	local attempt=1
	local delay=5
	local exit_code=0

	while true; do
		if "$@"; then
			return 0
		else
			exit_code=$?
		fi

		if ((attempt >= attempts)); then
			echo "Command failed after ${attempt} attempt(s): $*" >&2
			return "$exit_code"
		fi

		echo "Command failed with exit code ${exit_code}: $*" >&2
		echo "Retrying (${attempt}/${attempts}) in ${delay}s..." >&2
		sleep "$delay"
		attempt=$((attempt + 1))
		delay=$((delay * 2))
	done
}

if ! command -v nix >/dev/null 2>&1; then
	curl -fsSL https://nixos.org/nix/install -o /tmp/nix-install.sh
	sh /tmp/nix-install.sh --no-daemon --yes
fi

if [[ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]]; then
	# shellcheck disable=SC1091
	. "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi

mkdir -p "${HOME}/.config/nix"
cat >"${HOME}/.config/nix/nix.conf" <<'EOF'
experimental-features = nix-command flakes
accept-flake-config = true
extra-substituters = https://devenv.cachix.org
extra-trusted-public-keys = devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=
max-jobs = auto
EOF

devenv_revision="$(
	nix eval --impure --raw --expr '
		let
			lock = builtins.fromJSON (builtins.readFile ./devenv.lock);
			revision = lock.nodes.devenv.locked.rev;
		in
			if builtins.match "^[0-9a-f]{40}$" revision != null then
				revision
			else
				throw "devenv.lock does not contain an immutable Devenv revision"
	'
)"

if ! command -v devenv >/dev/null 2>&1 || [[ "$(devenv version)" != *"${devenv_revision:0:7}"* ]]; then
	retry 3 nix profile add --accept-flake-config "github:cachix/devenv/${devenv_revision}#devenv"
fi

if [[ -x "${HOME}/.nix-profile/bin/nix" ]]; then
	sudo ln -sfn "${HOME}/.nix-profile/bin/nix" /usr/local/bin/nix
fi
if [[ -x "${HOME}/.nix-profile/bin/devenv" ]]; then
	sudo ln -sfn "${HOME}/.nix-profile/bin/devenv" /usr/local/bin/devenv
fi

sudo tee /etc/profile.d/nix-devenv.sh >/dev/null <<'EOF'
if [[ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]]; then
	. "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi
EOF

retry 3 devenv shell -- install:all
retry 3 devenv shell -- bash -euo pipefail -c 'pnpm install --frozen-lockfile && cargo fetch --locked'
