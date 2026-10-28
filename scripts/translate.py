import json
import re
import subprocess
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from concurrent.futures import as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class Language:
    code: str
    name: str


LANGUAGES = [
    Language("sv", "Swedish"),
    Language("es", "Spanish"),
    Language("de", "German"),
    Language("fi", "Finnish"),
    Language("fr", "French"),
    Language("pl", "Polish"),
    Language("vi", "Vietnamese"),
    Language("nl", "Dutch"),
    Language("zh-Hans", "Simplified Chinese"),
    Language("zh-Hant", "Traditional Chinese (Taiwan)"),
    Language("tr", "Turkish"),
    Language("pt-BR", "Brazilian Portuguese"),
    Language("pt-PT", "European Portuguese"),
    Language("id", "Indonesian"),
    Language("it", "Italian"),
    Language("ja", "Japanese"),
    Language("hi", "Hindi"),
    Language("ko", "Korean"),
    Language("ru", "Russian"),
    Language("uk", "Ukrainian"),
    Language("sk", "Slovak"),
]

FRONTEND = Path(__file__).resolve().parent.parent / "frontend"
SOURCES = FRONTEND / "src"
LOCALES = SOURCES / "locales"
LITERAL = r"""(?:"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*')"""
CALL_RE = re.compile(rf"(?<![\w.$])(?:t|tRich)\(\s*({LITERAL}(?:\s*\+\s*{LITERAL})*)\s*[,)]")
ANY_CALL_RE = re.compile(r"(?<![\w.$])(?:t|tRich)\(")
TITLE_RE = re.compile(r"<title>(.*?)</title>")
PLACEHOLDER_RE = re.compile(r"\{\w+\}")
BATCH_SIZE = 25
WORKERS = 4

SYSTEM_PROMPT = """\
You translate Moblin Cloud, the website of Moblin, a free and open source iOS app for IRL live \
streaming to Twitch, YouTube, Kick and others, where streamers get a page on which viewers tip \
them and spin a wheel of luck, paying with PayPal or Stripe, and a dashboard where they manage it \
and their Moblin devices. Use the \
terminology a streamer or viewer would expect in each language and a friendly, casual tone, \
keep translations about as long as the English original, and match its capitalization style. \
Keep technical terms and product names (Moblin, Moblin Cloud, Mobcam, Moblink, Twitch, PayPal, \
Apple Music, bitrate, SRT, RTMP, OBS, ...) untranslated when that is what native speakers use. \
Placeholders in curly braces such as {name} and {link} must appear in every translation exactly \
as in the English string, untranslated and with the same count; move them if grammar requires \
it. Preserve newlines and leading/trailing whitespace. Each string comes with the source files \
it is used in; use them to pick the right meaning but never translate them."""


@dataclass
class String:
    english: str
    files: list[str]
    missing: list[Language]


@dataclass
class Batch:
    strings: list[String]

    def language_names(self) -> list[str]:
        names: set[str] = set()
        for string in self.strings:
            for language in string.missing:
                names.add(language.name)
        return sorted(names)


def decode_literal(literal: str) -> str:
    if literal.startswith("'"):
        literal = '"' + literal[1:-1].replace("\\'", "'").replace('"', '\\"') + '"'
    return json.loads(literal.replace("\\'", "'"))


def extract() -> dict[str, list[str]]:
    strings: dict[str, list[str]] = defaultdict(list)
    for path in sorted(FRONTEND.glob("**/*.html")):
        if "node_modules" in path.parts or "dist" in path.parts:
            continue
        for title in TITLE_RE.findall(path.read_text()):
            strings[title].append(str(path.relative_to(FRONTEND)))
    for path in sorted(SOURCES.glob("**/*.ts*")):
        source = path.read_text()
        name = str(path.relative_to(FRONTEND))
        literals = 0
        for match in CALL_RE.finditer(source):
            english = "".join(decode_literal(part) for part in re.findall(LITERAL, match.group(1)))
            if name not in strings[english]:
                strings[english].append(name)
            literals += 1
        if path.name != "i18n.ts" and literals != len(ANY_CALL_RE.findall(source)):
            print(f"{name}: t() and tRich() must be called with a string literal")
    return strings


def load(language: Language) -> dict[str, str]:
    path = LOCALES / f"{language.code}.json"
    if not path.exists():
        return {}
    translations: dict[str, str] = json.loads(path.read_text())
    return translations


def store(language: Language, translations: dict[str, str]) -> None:
    LOCALES.mkdir(exist_ok=True)
    path = LOCALES / f"{language.code}.json"
    content = json.dumps(dict(sorted(translations.items())), indent=2, ensure_ascii=False)
    path.write_text(content + "\n")


def placeholders(text: str) -> list[str]:
    return sorted(PLACEHOLDER_RE.findall(text))


def translate_batch(batch: Batch) -> dict[int, dict[str, Any]]:
    language_names = batch.language_names()
    schema = {
        "type": "object",
        "properties": {
            "translations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        **{name: {"type": "string"} for name in language_names},
                    },
                    "required": ["id", *language_names],
                },
            }
        },
        "required": ["translations"],
    }
    strings = [
        {"id": index, "english": string.english, "files": string.files}
        for index, string in enumerate(batch.strings)
    ]
    prompt = (
        f"Translate each English string below to {', '.join(language_names)}. "
        "Return one entry per id.\n\n" + json.dumps(strings, indent=2, ensure_ascii=False)
    )
    result = subprocess.run(
        [
            "claude",
            "-p",
            "--model",
            "sonnet",
            "--output-format",
            "json",
            "--tools",
            "",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--setting-sources",
            "",
            "--system-prompt",
            SYSTEM_PROMPT,
            "--json-schema",
            json.dumps(schema),
            prompt,
        ],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=True,
    )
    output = json.loads(result.stdout)
    if output.get("is_error"):
        raise RuntimeError(output.get("result"))
    return {entry["id"]: entry for entry in output["structured_output"]["translations"]}


def apply(batch: Batch, translations: dict[int, dict[str, Any]], catalogs: dict[str, dict[str, str]]) -> None:
    for index, string in enumerate(batch.strings):
        entry = translations.get(index)
        if entry is None:
            print(f'Missing translation of "{string.english}"')
            continue
        for language in string.missing:
            translated = entry.get(language.name)
            if translated is None:
                continue
            if placeholders(translated) != placeholders(string.english):
                print(f'Wrong placeholders in {language.name} translation of "{string.english}"')
                continue
            catalogs[language.code][string.english] = translated


def main() -> None:
    strings = extract()
    catalogs = {language.code: load(language) for language in LANGUAGES}
    for catalog in catalogs.values():
        for english in list(catalog):
            if english not in strings:
                del catalog[english]
    pending = []
    for english, files in strings.items():
        missing = [language for language in LANGUAGES if english not in catalogs[language.code]]
        if missing:
            pending.append(String(english, files, missing))
    for language in LANGUAGES:
        store(language, catalogs[language.code])
    batches = [Batch(pending[i : i + BATCH_SIZE]) for i in range(0, len(pending), BATCH_SIZE)]
    print(f"Translating {len(pending)} of {len(strings)} strings in {len(batches)} batches")
    with ThreadPoolExecutor(max_workers=WORKERS) as executor:
        futures = {executor.submit(translate_batch, batch): batch for batch in batches}
        for number, future in enumerate(as_completed(futures), 1):
            batch = futures[future]
            try:
                apply(batch, future.result(), catalogs)
            except Exception as error:
                print(f'Batch starting with "{batch.strings[0].english}" failed: {error}')
                continue
            for language in LANGUAGES:
                store(language, catalogs[language.code])
            print(f"Batch {number}/{len(batches)} done")


main()
