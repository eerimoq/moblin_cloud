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
# Signs in as the fake Twitch user admin (an admin), pro (a pro streamer) or normal, each with its own account.
backend-run role="admin" *args:
	#!/usr/bin/env bash
	set -euo pipefail
	case "{{role}}" in
		admin|pro|normal) ;;
		*) echo "Unknown role '{{role}}', expected admin, pro or normal." >&2; exit 1 ;;
	esac
	cd backend
	cargo build
	DATABASE="${DATABASE:-./database.sqlite}" cargo run -q -- --listen 0.0.0.0:8080 --allow-unattested --fake-twitch-sign-in {{role}} --admins fake-admin --pro-streamers fake-pro --public-url http://localhost:5173/api --frontend-url http://localhost:5173 {{args}} &
	trap 'kill $!' EXIT
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
