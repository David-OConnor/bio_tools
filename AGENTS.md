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