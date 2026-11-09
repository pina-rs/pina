#!/usr/bin/env bash
# Bootstrap Nix and devenv on a Linux Cloud Agent VM.
#
# Pina's development shell is the lockfile in devenv.lock, realized by
# devenv.nix. Cloud Agent VMs run under tini rather than systemd, so Nix is
# installed single-user. The devenv package is the immutable revision from
# devenv.lock (the same pin CI uses), substituted from devenv.cachix.org
# rather than built from source. macOS host-linker overrides and the Windows
# CLI job are outside this image.

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

# Single-user Nix lives in the user profile. A second run finds that binary
# even when the current shell has not sourced the profile yet.
if [[ ! -x "${HOME}/.nix-profile/bin/nix" ]] && ! command -v nix >/dev/null 2>&1; then
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
	# `install` is Nix 2.35's alias for `profile add`. The flake URL is the
	# cachix/devenv pin recorded in devenv.lock, not a floating nixpkgs copy.
	retry 3 nix profile install --accept-flake-config "github:cachix/devenv/${devenv_revision}#devenv"
fi

# Login shells source /etc/profile.d. Non-login shells do not, so wrappers in
# /usr/local/bin (on the default PATH) source the Nix profile and exec the
# real binaries. That covers `bash -c` as well as `bash -lc`.
install_wrapper() {
	local name="$1"
	sudo tee "/usr/local/bin/${name}" >/dev/null <<EOF
#!/bin/sh
if [ -n "\${HOME-}" ] && [ -f "\${HOME}/.nix-profile/etc/profile.d/nix.sh" ]; then
	. "\${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi
exec "\${HOME}/.nix-profile/bin/${name}" "\$@"
EOF
	sudo chmod 755 "/usr/local/bin/${name}"
}

install_wrapper nix
install_wrapper devenv

sudo tee /etc/profile.d/nix-devenv.sh >/dev/null <<'EOF'
if [[ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]]; then
	. "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi
EOF

# Realize the devenv shell once so later agents reuse the cached closure.
retry 3 devenv shell -- true
retry 3 devenv shell -- install:all
retry 3 devenv shell -- bash -euo pipefail -c 'pnpm install --frozen-lockfile && cargo fetch --locked'
