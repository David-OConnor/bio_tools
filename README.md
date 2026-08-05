# Bio tools

An interface for running arbitrary CLI applications for biology and chemistry. Focuses on tools with permissive
licencing, and ones which are most popular. Handles installing these tools by downloading them, following official
installation procedures.

## Generic interfaces
Provides an interface for input and output. This abstracts over the differences between tools, so applications
can add many of them without repeating code


## Installation
Handles installing applications. Details depend on the tool; some work by placing application executables in the
appropriate places. Since many of these use Python, it uses [uv](https://docs.astral.sh/uv/) to set up isolated environments 


## Example uses
- Building a GUI (Web or native) to these tools
- Setting up an API to programmatically interface.


## Python bindings
Available for both Python projects via `PyO3` nad `maturin`; available on PyPi.

`pip install bio_tools`