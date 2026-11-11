python_dirs := "tests scripts"

config_dir := ".config"

[private]
default:
	@just --list

# Signing in goes through the fake Twitch in tests/utils/fake_twitch.py and the fake Kick in
# tests/utils/fake_kick.py, which sign in as any made-up user, each with its own account until
# linked: admin is an admin, premium one to make premium on the admin page, normal neither, on both.
[group('dev')]
[doc("Runs the backend, the frontend and the fake Moblin as the admin user's, at http://localhost:5173, with the given sign in platforms, like just dev kick, or none with just dev \"\".")]
dev platforms="twitch,kick":
	python3 -m scripts.dev all "{{platforms}}"

[group('dev')]
[doc("Runs just dev with the compact landing page and an instance name, like just dev-compact \"My workspace\" kick.")]
dev-compact name="My workspace" platforms="twitch,kick":
	LANDING_PAGE=compact INSTANCE_NAME="{{name}}" python3 -m scripts.dev all "{{platforms}}"

[group('dev')]
[doc("Runs the backend with the fake Twitch and Kick, or those in --platforms, like --platforms kick, and none with --platforms \"\". Other arguments go to the backend.")]
dev-backend *args:
	python3 -m scripts.dev backend {{args}}

[group('dev')]
[doc("Runs the frontend's dev server at http://localhost:5173, talking to dev-backend.")]
dev-frontend:
	python3 -m scripts.dev frontend

# Always connects as the same device, so the dashboard lists it once.
[group('dev')]
[doc("Connects the fake Moblin to dev-backend as the admin user's Moblin. Pass --platform kick if signed in with Kick, --device-id for another device.")]
dev-fake-moblin *args:
	python3 -m scripts.dev fake-moblin {{args}}

[group('dev')]
[doc("Runs mobtop, the terminal dashboard, against dev-backend through dev-frontend, like the browser, so run just dev first.")]
dev-mobtop *args:
	python3 -m scripts.dev mobtop {{args}}

[group('check')]
style:
	cd frontend && oxfmt
	cd backend && cargo fmt
	cd mobtop && cargo fmt
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}}
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}}

[group('check')]
style-check:
	cd frontend && oxfmt --check
	cd backend && cargo fmt --check
	cd mobtop && cargo fmt --check
	isort --settings-path {{config_dir}}/isort.cfg {{python_dirs}} --check
	ruff format --config {{config_dir}}/ruff.toml {{python_dirs}} --check

[group('check')]
lint:
	cd frontend && oxlint --deny-warnings
	cd backend && cargo clippy --all-targets -- -D warnings
	cd mobtop && cargo clippy --all-targets -- -D warnings
	pylint --rcfile {{config_dir}}/pylintrc {{python_dirs}}
	ruff check --config {{config_dir}}/ruff.toml {{python_dirs}}
	mypy --config-file {{config_dir}}/mypy.ini {{python_dirs}}

[group('check')]
spell-check:
	codespell

[group('check')]
test:
	cd frontend && npm test
	cd backend && cargo test
	cd mobtop && cargo test

[group('check')]
[doc("Runs the backend system tests, see tests/README.md.")]
system-test *args:
	python3 -m tests.test {{args}}

[group('check')]
[doc("Runs the backend system tests and reports the backend's code coverage, see tests/README.md.")]
system-test-coverage *args:
	python3 -m tests.test --coverage {{args}}

[group('maintenance')]
[doc("Lists the Moblin commits the scene editor has not ported yet, from ../moblin, see scripts/moblin_ports.json.")]
moblin-drift *args:
	python3 scripts/moblin_drift.py {{args}}

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
