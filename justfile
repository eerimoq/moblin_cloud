python_dirs := "tests scripts"

config_dir := ".config"

style:
	cd frontend && oxfmt
	cd backend && cargo fmt
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}}
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}}

style-check:
	cd frontend && oxfmt --check
	cd backend && cargo fmt --check
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}} --check
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}} --check

lint:
	cd frontend && oxlint --deny-warnings
	cd backend && cargo clippy --all-targets -- -D warnings
	pylint --rcfile {{config_dir}}/pylintrc {{python_dirs}}
	ruff check --config {{config_dir}}/ruff.toml {{python_dirs}}
	mypy --config-file {{config_dir}}/mypy.ini {{python_dirs}}

spell-check:
	codespell

test:
	cd backend && cargo test

# Runs the backend system tests, see tests/README.md.
system-test *args:
	python3 -m tests.test {{args}}

# Copies Moblin settings that connect its remote control streamer to the system tests.
system-test-generate-device-settings-clipboard:
	python3 -m tests.generate_device_settings

system-test-generate-device-settings-stdout:
	python3 -m tests.generate_device_settings --force-stdout

npm-latest-args := "python -c \"import json,sys; print(' '.join(f'{d}@latest' for d in json.load(open('package.json'))[sys.argv[1]]))\""

update-dependencies:
	cd frontend && npm install $({{npm-latest-args}} dependencies)
	cd frontend && npm install --save-dev $({{npm-latest-args}} devDependencies)

frontend-run:
	#!/usr/bin/env bash
	set -euo pipefail
	cd frontend
	npm run dev

# Runs the backend and fills it with a few streamers, for developing the website.
# Signing in goes through the fake Twitch in tests/utils/fake_twitch.py, which signs in as any made-up
# user, each with its own account: admin is an admin, premium a premium account, normal neither.
backend-run *args:
	#!/usr/bin/env bash
	set -euo pipefail
	python3 -m tests.utils.fake_twitch --port 8081 &
	trap 'kill $(jobs -p)' EXIT
	cd backend
	cargo build
	DATABASE="${DATABASE:-./database.sqlite}" cargo run -q -- --listen 0.0.0.0:8080 --allow-unattested --twitch-sign-in-url http://localhost:8081 --admins fake-admin --premium-accounts fake-premium --public-url http://localhost:5173/api --frontend-url http://localhost:5173 {{args}} &
	until curl -sf localhost:8080/streamers > /dev/null; do sleep 0.2; done
	just backend-seed
	wait

# Runs the backend on all interfaces, accepting attested live posts from Moblin builds
# installed by Xcode, for testing with a phone on the same network.
backend-run-attested *args:
	cd backend && cargo run -q -- --listen 0.0.0.0:8080 --app-attest-environment development {{args}}

# Tells the backend that a few made-up streamers went live.
backend-seed:
	python3 scripts/seed_backend.py

# Keeps telling the backend that new made-up streamers went live, for testing that the website updates.
backend-seed-forever interval="2":
	python3 scripts/seed_backend.py --forever {{interval}}

# Connects the fake Moblin to the running `just backend-run` as the admin user's Moblin, with a playlist to show in the player.
fake-moblin-run *args:
	#!/usr/bin/env bash
	set -euo pipefail
	key=$(sqlite3 "backend/${DATABASE:-./database.sqlite}" "select key from devices where account = 'fake-admin' and enabled = 1 order by position limit 1")
	if [ -z "$key" ]; then
		echo "No Moblin device for the admin user. Sign in as admin at http://localhost:5173/dashboard/ first." >&2
		exit 1
	fi
	python3 -m tests.utils.fake_moblin http://localhost:8080 --key "$key" {{args}}

backend-docker-build:
	docker build -t moblin-website-backend backend

backend-docker-run *args: backend-docker-build
	docker run --rm -it --init -p 8080:8080 moblin-website-backend {{args}}

# Regenerates the settings reference on the docs page from a Moblin checkout.
docs-generate moblin="../moblin":
	python3 scripts/generate_docs.py {{moblin}} frontend/src/data/docs/settings.json
	cd frontend && oxfmt src/data/docs/settings.json

# Translates new strings of the website to all languages with Claude.
machine-translate:
	python3 scripts/translate.py
	cd frontend && oxfmt src/locales

# Renders every trading card as a 300 dpi PNG with bleed, sized for poker cards at makeplayingcards.com.
cards-print out="cards-print":
	cd frontend && npm run build
	cd frontend && node scripts/print-cards.ts {{absolute_path(out)}}

# Pretends to be a Moblin streamer connected over remote control, for developing song requests on the page.
backend-fake-streamer *key:
	python3 scripts/remote_control_streamer.py {{key}}
