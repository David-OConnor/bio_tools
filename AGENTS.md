## Keeping in sync with `bio_web`
When making changes to a Tool's fields, you may also need to make changes to `../bio_web`'s associated
page under its `templates/tools` folder. Keep it in sync with this project. 

`bio_web` or "Bio web"
refers to `../bio_web`.


## Fields and presets for each tool
Field definitions live under `src/tool_definitions/fields` as `.json` files. This, and any data
in `.rs` files in `tool_definitions` should match their official documentation, examples, API, field descriptions,
and input/output
descriptions as closely as possible. We try to maintain accurate links to these in `SpecData` *_url fields.

See also `bio_web`: `main/tools` `.py` files: These may need to be kept in sync with the official resources.


## Installed environments
- Files in tool environments may be hard links shared with uv's cache and other environments
  (see `src/install/package_cache.rs`). Install steps that change an installed file must write a
  new file and rename it over the old one, never edit it in place.
- When removing a tool, add its slug to `RETIRED` in `src/install/uninstall.rs` so existing
  installations drop its environment.


## Unit tests
Do not add unit tests permanently. You may add them transiently e.g. while adding or updating
functionality; clean them up after.


## Code format
Add appropriate line breaks in code; I have noticed LLMs tend to not use line breaks, which makes it tough for me to read and edit code.

## Readme and documentation prose style

Don't edit the readme unless I explicitly ask you to, or to fix things in it that you changed the behavior of. Any text you write in documentation comments in code (Or the REAMDE after I give you permission) should be terse. The dialect you prefer to write in is generally too verbose and diffulct to read; don't do that.