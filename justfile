python_dirs := "tests scripts"

config_dir := ".config"

[private]
default:
	@just --list

# Signing in goes through the fake Twitch in tests/utils/fake_twitch.py and the fake Kick in
# tests/utils/fake_kick.py, which sign in as any made-up user, each with its own account until
# linked: admin is an admin, premium a premium account, normal neither, on both.
[group('dev')]
[doc("Runs the backend, the frontend and the fake Moblin as the admin user's, at http://localhost:5173.")]
dev:
	#!/usr/bin/env bash
	set -euo pipefail
	trap 'trap "" INT TERM; kill $(jobs -p) 2> /dev/null' EXIT
	just dev-backend &
	until curl -sf localhost:8080/icons/plain > /dev/null; do sleep 0.2; done
	just dev-frontend &
	cookies=$(mktemp)
	authorize=$(curl -sf -c "$cookies" -o /dev/null -w '%{redirect_url}' localhost:8080/auth/twitch/login)
	callback=$(curl -sf -o /dev/null -w '%{redirect_url}' -G "$authorize" --data-urlencode login=admin)
	curl -sf -b "$cookies" -o /dev/null "${callback/localhost:5173\/api/localhost:8080}"
	rm "$cookies"
	just dev-fake-moblin &
	wait

[group('dev')]
[doc("Runs the backend with the fake Twitch and Kick.")]
dev-backend *args:
	#!/usr/bin/env bash
	set -euo pipefail
	python3 -m tests.utils.fake_twitch --port 8081 &
	python3 -m tests.utils.fake_kick --port 8082 &
	trap 'trap "" INT TERM; kill $(jobs -p) 2> /dev/null' EXIT
	cd backend
	cargo build
	DATABASE="${DATABASE:-./database.sqlite}" cargo run -q -- --listen 0.0.0.0:8080 --allow-unattested --twitch-sign-in-url http://localhost:8081 --kick-sign-in-url http://localhost:8082 --admins 847161455,kick:33069881 --premium-accounts 325813636,kick:57017113 --backend-url http://localhost:5173/api --frontend-url http://localhost:5173 {{args}} &
	wait

[group('dev')]
[doc("Runs the frontend's dev server at http://localhost:5173, talking to dev-backend.")]
dev-frontend:
	cd frontend && npm run dev

# Always connects as the same device, so the dashboard lists it once.
[group('dev')]
[doc("Connects the fake Moblin to dev-backend as the admin user's Moblin. Pass --device-id for another device.")]
dev-fake-moblin *args:
	#!/usr/bin/env bash
	set -euo pipefail
	key=$(sqlite3 "backend/${DATABASE:-./database.sqlite}" "select moblin_key from accounts join identities on identities.account = accounts.id where platform = 'twitch' and login = 'admin'")
	if [ -z "$key" ]; then
		echo "No Moblin Cloud key for the admin user. Sign in as admin at http://localhost:5173/dashboard/ first." >&2
		exit 1
	fi
	python3 -m tests.utils.fake_moblin http://localhost:8080 --key "$key" --device-id FAKE-MOBLIN {{args}}

[group('check')]
style:
	cd frontend && oxfmt
	cd backend && cargo fmt
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}}
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}}

[group('check')]
style-check:
	cd frontend && oxfmt --check
	cd backend && cargo fmt --check
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}} --check
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}} --check

[group('check')]
lint:
	cd frontend && oxlint --deny-warnings
	cd backend && cargo clippy --all-targets -- -D warnings
	pylint --rcfile {{config_dir}}/pylintrc {{python_dirs}}
	ruff check --config {{config_dir}}/ruff.toml {{python_dirs}}
	mypy --config-file {{config_dir}}/mypy.ini {{python_dirs}}

[group('check')]
spell-check:
	codespell

[group('check')]
test:
	cd backend && cargo test

[group('check')]
[doc("Runs the backend system tests, see tests/README.md.")]
system-test *args:
	python3 -m tests.test {{args}}

[group('check')]
[doc("Copies Moblin settings that connect it to the system tests to the clipboard. Pass --force-stdout to print them.")]
system-test-device-settings *args:
	python3 -m tests.generate_device_settings {{args}}

npm-latest-args := "python -c \"import json,sys; print(' '.join(f'{d}@latest' for d in json.load(open('package.json'))[sys.argv[1]]))\""

[group('maintenance')]
[doc("Updates the frontend's npm dependencies to their latest versions.")]
update-dependencies:
	cd frontend && npm install $({{npm-latest-args}} dependencies)
	cd frontend && npm install --save-dev $({{npm-latest-args}} devDependencies)

[group('maintenance')]
[doc("Translates new strings of the frontend to all languages with Claude.")]
machine-translate:
	python3 scripts/translate.py
	cd frontend && oxfmt src/locales
